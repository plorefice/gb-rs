use bitflags::bitflags;
use crossbeam::queue::ArrayQueue;

use super::dbg;
use super::IoReg;
use super::{InterruptSource, IrqSource};
use super::{MemR, MemW};

use std::sync::Arc;

const CLK_64_RELOAD: u32 = 4_194_304 / 64;
const CLK_128_RELOAD: u32 = 4_194_304 / 128;
const CLK_256_RELOAD: u32 = 4_194_304 / 256;

bitflags! {
    // NRx0 - Channel x Sweep register (R/W)
    struct NRx0: u8 {
        const SWEEP_TIME  = 0b_0111_0000;
        const SWEEP_NEG   = 0b_0000_1000;
        const SWEEP_SHIFT = 0b_0000_0111;

        const WAVE_DAC_ON = 0b_1000_0000;
    }
}

bitflags! {
    // NRx1 - Channel x Sound Length/Wave Pattern Duty (R/W)
    struct NRx1: u8 {
        const WAVE_DUTY = 0b_1100_0000;
        const SOUND_LEN = 0b_0011_1111;

        const WAVE_SOUND_LEN = 0b_1111_1111;
    }
}

bitflags! {
    // NRx2 - Channel x Volume Envelope (R/W)
    struct NRx2: u8 {
        const START_VOL  = 0b_1111_0000;
        const ENV_DIR    = 0b_0000_1000;
        const ENV_PERIOD = 0b_0000_0111;

        const DAC_ON     = 0b_1111_1000;

        const WAVE_VOLUME = 0b_0110_0000;
    }
}

bitflags! {
    // NRx4 - Channel x Frequency hi data (R/W)
    struct NRx4: u8 {
        const TRIGGER = 0b_1000_0000;
        const LEN_EN  = 0b_0100_0000;
        const FREQ_HI = 0b_0000_0111;
    }
}

bitflags! {
    // NR50 - Channel control / ON-OFF / Volume (R/W)
    struct NR50: u8 {
        const VIN_L_EN  = 0b_1000_0000;
        const LEFT_VOL  = 0b_0111_0000;
        const VIN_R_EN  = 0b_0000_1000;
        const RIGHT_VOL = 0b_0000_0111;
    }
}

bitflags! {
    // NR51 - Selection of Sound output terminal (R/W)
    struct NR51: u8 {
        const OUT4_L = 0b_1000_0000;
        const OUT3_L = 0b_0100_0000;
        const OUT2_L = 0b_0010_0000;
        const OUT1_L = 0b_0001_0000;
        const OUT4_R = 0b_0000_1000;
        const OUT3_R = 0b_0000_0100;
        const OUT2_R = 0b_0000_0010;
        const OUT1_R = 0b_0000_0001;
    }
}

bitflags! {
    // NR52 - Sound on/off
    struct NR52: u8 {
        const PWR_CTRL = 0b_1000_0000;
        const OUT_4_EN = 0b_0000_1000;
        const OUT_3_EN = 0b_0000_0100;
        const OUT_2_EN = 0b_0000_0010;
        const OUT_1_EN = 0b_0000_0001;
    }
}

/// A sound channel able to produce quadrangular wave patterns
/// with optional sweep and envelope functions.
struct ToneChannel {
    // Channel registers
    nrx0: NRx0,
    nrx1: NRx1,
    nrx2: NRx2,
    nrx3: IoReg<u8>,
    nrx4: NRx4,

    // Internal state and timer counter
    enabled: bool,
    timer_counter: u32,

    // Length counter
    len_ctr: u32,

    // Frequency sweep unit
    sweep_support: bool,
    sweep_enabled: bool,
    sweep_freq_shadow: u32,
    sweep_timer: u8,

    // Volume control
    volume: i16,
    vol_ctr: u8,
    vol_env_enabled: bool,

    // Channel output fed as input to the mixer
    waveform_level: i16,
}

impl ToneChannel {
    /// Creates a tone channel with the initial register state provided.
    fn new(
        nrx0: NRx0,
        nrx1: NRx1,
        nrx2: NRx2,
        nrx3: IoReg<u8>,
        nrx4: NRx4,
        sweep_support: bool,
    ) -> ToneChannel {
        ToneChannel {
            nrx0,
            nrx1,
            nrx2,
            nrx3,
            nrx4,

            enabled: false,
            timer_counter: 0,

            len_ctr: 0,

            sweep_support,
            sweep_enabled: false,
            sweep_freq_shadow: 0,
            sweep_timer: 0,

            volume: 0,
            vol_ctr: 0,
            vol_env_enabled: false,

            waveform_level: 1,
        }
    }

    /// Advances the internal timer state by one M-cycle.
    fn tick(&mut self) {
        let period = self.get_period();

        // The timer generates an output clock every N input clocks,
        // where N is the timer's period.
        if self.timer_counter < 4 {
            self.timer_counter = period - self.timer_counter;
        } else {
            self.timer_counter -= 4;
        }

        // Duty   Waveform    Ratio
        // -------------------------
        // 0      00000001    12.5%
        // 1      10000001    25%
        // 2      10000111    50%
        // 3      01111110    75%
        let threshold = match (self.nrx1 & NRx1::WAVE_DUTY).bits() >> 6 {
            0 => period / 8,
            1 => period / 4,
            2 => period / 2,
            3 => period * 3 / 4,
            _ => unreachable!(),
        };

        self.waveform_level = if self.timer_counter < threshold { 1 } else { 0 };
    }

    /// Advances the frequency sweep unit by 1/128th of a second.
    fn tick_freq_sweep(&mut self) {
        let shift = (self.nrx0 & NRx0::SWEEP_SHIFT).bits();
        let period = (self.nrx0 & NRx0::SWEEP_TIME).bits() >> 4;

        if !self.sweep_support || !self.sweep_enabled || period == 0 {
            return;
        }

        self.sweep_timer -= 1;

        // Sweep timer expired -> do sweep
        if self.sweep_timer == 0 {
            // Reload internal timer
            self.sweep_timer = period;

            // Compute new frequency
            let new_freq = self.do_sweep_calc();

            // Use it if it is less than 2048 and the shift is not zero
            if new_freq < 2048 && shift != 0 {
                self.sweep_freq_shadow = u32::from(new_freq);
                self.set_frequency(new_freq);

                // Frequency calculations and overflow check are run again,
                // but this time the result is not used.
                self.do_sweep_calc();
            }
        }
    }

    /// Advances the volume envelope unit by 1/64th of a second.
    fn tick_vol_env(&mut self) {
        let period = (self.nrx2 & NRx2::ENV_PERIOD).bits();

        // When the timer generates a clock and the envelope period is not zero,
        // a new volume is calculated by adding or subtracting 1 from the current volume.
        if self.vol_env_enabled && period > 0 {
            self.nrx2 = (self.nrx2 & !NRx2::ENV_PERIOD) | NRx2::from_bits_truncate(period - 1);

            let new_volume = if self.nrx2.contains(NRx2::ENV_DIR) {
                self.volume + 1
            } else {
                self.volume - 1
            };

            // If this new volume within the 0 to 15 range, the volume is updated,
            // otherwise it is left unchanged and no further automatic increments/decrements
            // are made to the volume until the channel is triggered again.
            if new_volume <= 15 {
                self.volume = new_volume;
            } else {
                self.vol_env_enabled = false;
            }
        }
    }

    /// Advances the length counter unit by 1/256th of a second.
    fn tick_len_ctr(&mut self) {
        // When clocked while enabled by NRx4 and the counter is not zero, length is decremented
        if self.nrx4.contains(NRx4::LEN_EN) && self.len_ctr != 0 {
            self.len_ctr -= 1;

            // If it becomes zero, the channel is disabled
            if self.len_ctr == 0 {
                self.enabled = false;
            }
        }
    }

    /// Performs frequency sweep calculations and overflow check
    fn do_sweep_calc(&mut self) -> u16 {
        let neg = self.nrx0.contains(NRx0::SWEEP_NEG);
        let shift = (self.nrx0 & NRx0::SWEEP_SHIFT).bits();

        // Sweep formula: X(t) = X(t-1) +/- X(t-1)/2^n

        let mut new_freq = self.sweep_freq_shadow >> shift;

        if neg {
            new_freq = self.sweep_freq_shadow - new_freq;
        } else {
            new_freq += self.sweep_freq_shadow;
        }

        // Overflow check: if the new frequency is greater than 2047, the channel is disabled.
        if new_freq >= 2048 {
            self.enabled = false;
        }

        new_freq as u16
    }

    /// Returns the channel's period.
    fn get_period(&self) -> u32 {
        u32::from(2048 - self.get_frequency()) << 5
    }

    /// Sets the channel's current tone frequency.
    fn set_frequency(&mut self, freq: u16) {
        self.nrx3.0 = freq as u8;
        self.nrx4 = (self.nrx4 & !NRx4::FREQ_HI) | NRx4::from_bits_truncate((freq >> 8) as u8);
    }

    /// Returns the channel's current tone frequency.
    fn get_frequency(&self) -> u16 {
        let hi = u16::from((self.nrx4 & NRx4::FREQ_HI).bits());
        let lo = u16::from(self.nrx3.0);
        (hi << 8) | lo
    }

    /// Returns the channel's current volume.
    fn get_volume(&self) -> i16 {
        i16::from(self.enabled) * self.volume
    }

    /// Returns the channel's current output level, ready to be fed to the mixer.
    fn get_channel_out(&self) -> i16 {
        if self.dac_on() {
            (self.waveform_level * 2 * self.get_volume() as i16) - 15
        } else {
            0
        }
    }

    /// Returns true if the channels DAC is on, false otherwise.
    fn dac_on(&self) -> bool {
        (self.nrx2 & NRx2::DAC_ON).bits() != 0
    }

    /// Handles a write to the NRx4 register.
    fn write_to_nr4(&mut self, val: u8) {
        self.nrx4 = NRx4::from_bits_truncate(val);

        // When a TRIGGER occurs, a number of things happen
        if self.nrx4.contains(NRx4::TRIGGER) {
            // Channel is enabled
            self.enabled = true;

            // If length counter is zero, it is set to 64 (256 for wave channel)
            if self.len_ctr == 0 {
                self.len_ctr = 64;
            }

            // Frequency timer is reloaded with period
            self.timer_counter = self.get_period();

            // Volume envelope timer is reloaded with period and
            // channel volume is reloaded from NRx2.
            self.volume = i16::from((self.nrx2 & NRx2::START_VOL).bits() >> 4);
            self.vol_ctr = (self.nrx2 & NRx2::ENV_PERIOD).bits();
            self.vol_env_enabled = true;

            // Square 1's frequency is copied to the shadow register, the sweep timer is reloaded,
            // the internal sweep enabled flag is adjusted and sweep calculations may be performed.
            let sweep_shift = (self.nrx0 & NRx0::SWEEP_SHIFT).bits();
            let sweep_period = (self.nrx0 & NRx0::SWEEP_TIME).bits() >> 4;

            self.sweep_freq_shadow = u32::from(self.get_frequency());
            self.sweep_timer = sweep_period;
            self.sweep_enabled = sweep_shift != 0 || sweep_period != 0;
            if sweep_shift != 0 {
                self.do_sweep_calc();
            }

            // Note that if the channel's DAC is off, after the above actions occur
            // the channel will be immediately disabled again.
            if !self.dac_on() {
                self.enabled = false;
            }
        }
    }
}

impl MemR for ToneChannel {
    fn read(&self, addr: u16) -> Result<u8, dbg::TraceEvent> {
        Ok(match addr {
            0 => {
                if self.sweep_support {
                    self.nrx0.bits() | 0x80
                } else {
                    0xFF
                }
            }
            1 => self.nrx1.bits() | 0x3F,
            2 => self.nrx2.bits(),
            3 => self.nrx3.0 | 0xFF,
            4 => self.nrx4.bits() | 0xBF,
            _ => unreachable!(),
        })
    }
}

impl MemW for ToneChannel {
    fn write(&mut self, addr: u16, val: u8) -> Result<(), dbg::TraceEvent> {
        match addr {
            0 => self.nrx0 = NRx0::from_bits_truncate(val),
            1 => {
                self.nrx1 = NRx1::from_bits_truncate(val);

                self.len_ctr = (self.nrx1 & NRx1::SOUND_LEN).bits() as u32;
            }
            2 => {
                self.nrx2 = NRx2::from_bits_truncate(val);

                if !self.dac_on() {
                    self.enabled = false;
                }
            }
            3 => self.nrx3.0 = val,
            4 => self.write_to_nr4(val),
            _ => unreachable!(),
        };

        Ok(())
    }
}

struct WaveChannel {
    // Channel registers
    nrx0: NRx0,
    nrx1: NRx1,
    nrx2: NRx2,
    nrx3: IoReg<u8>,
    nrx4: NRx4,

    // Internal state and timer counter
    enabled: bool,
    timer_counter: u32,

    // Length counter
    len_ctr: u32,

    // Wave functions
    wave_ram: [u8; 16],
    sample_buffer: u8,
    position_counter: usize,
}

impl Default for WaveChannel {
    fn default() -> WaveChannel {
        WaveChannel {
            nrx0: NRx0::from_bits_truncate(0x7F),
            nrx1: NRx1::from_bits_truncate(0xFF),
            nrx2: NRx2::from_bits_truncate(0x9F),
            nrx3: IoReg(0x00),
            nrx4: NRx4::from_bits_truncate(0xBF),

            enabled: false,
            timer_counter: 0,

            len_ctr: 0,

            wave_ram: [0; 16],
            sample_buffer: 0,
            position_counter: 0,
        }
    }
}

// TODO there is a lot of code shared between WaveChannel and ToneChannel.
// It should be aggregated without impacting too much on performance.
impl WaveChannel {
    /// Advances the internal timer state by one M-cycle.
    fn tick(&mut self) {
        // Every N input clocks, advance the position counter and latch the new sample.
        if self.timer_counter < 4 {
            self.timer_counter = self.get_period() - self.timer_counter;

            self.position_counter = (self.position_counter + 1) % 32;
            self.sample_buffer = self.wave_ram[self.position_counter >> 1];

            // Select the correct nibble
            if self.position_counter & 0x1 == 0 {
                self.sample_buffer >>= 4;
            } else {
                self.sample_buffer &= 0x0F;
            }
        } else {
            self.timer_counter -= 4;
        }
    }

    /// Advances the length counter unit by 1/256th of a second.
    fn tick_len_ctr(&mut self) {
        // When clocked while enabled by NRx4 and the counter is not zero, length is decremented
        if self.nrx4.contains(NRx4::LEN_EN) && self.len_ctr != 0 {
            self.len_ctr -= 1;

            // If it becomes zero, the channel is disabled
            if self.len_ctr == 0 {
                self.enabled = false;
            }
        }
    }

    /// Returns the channel's period.
    fn get_period(&self) -> u32 {
        u32::from(2048 - self.get_frequency()) << 1
    }

    /// Returns the channel's current tone frequency.
    fn get_frequency(&self) -> u16 {
        let hi = u16::from((self.nrx4 & NRx4::FREQ_HI).bits());
        let lo = u16::from(self.nrx3.0);
        (hi << 8) | lo
    }

    /// Returns the channel's current volume.
    fn get_volume(&self) -> u8 {
        u8::from(self.enabled) * ((self.nrx2 & NRx2::WAVE_VOLUME).bits() >> 5)
    }

    /// Returns the channel's current output level, ready to be fed to the mixer.
    fn get_channel_out(&self) -> i16 {
        if self.dac_on() {
            i16::from(self.sample_buffer >> (self.get_volume() - 1))
        } else {
            0
        }
    }

    /// Returns true if the channels DAC is on, false otherwise.
    fn dac_on(&self) -> bool {
        self.nrx0.contains(NRx0::WAVE_DAC_ON)
    }

    /// Handles a write to the NRx4 register.
    fn write_to_nr4(&mut self, val: u8) {
        self.nrx4 = NRx4::from_bits_truncate(val);

        // When a TRIGGER occurs, a number of things happen
        if self.nrx4.contains(NRx4::TRIGGER) {
            // Channel is enabled
            self.enabled = true;

            // If length counter is zero, it is set to 64 (256 for wave channel)
            if (self.nrx1 & NRx1::WAVE_SOUND_LEN).bits() == 0 {
                self.len_ctr = 255;
            }

            // Frequency timer is reloaded with period
            self.timer_counter = self.get_period();

            // Wave channel's position is set to 0 but sample buffer is NOT refilled
            self.position_counter = 0;

            // Note that if the channel's DAC is off, after the above actions occur
            // the channel will be immediately disabled again.
            if !self.dac_on() {
                self.enabled = false;
            }
        }
    }
}

impl MemR for WaveChannel {
    fn read(&self, addr: u16) -> Result<u8, dbg::TraceEvent> {
        Ok(match addr {
            0 => self.nrx0.bits() | 0x7F,
            1 => 0xFF,
            2 => self.nrx2.bits() | 0x9F,
            3 => 0xFF,
            4 => self.nrx4.bits() | 0xBF,
            _ => unreachable!(),
        })
    }
}

impl MemW for WaveChannel {
    fn write(&mut self, addr: u16, val: u8) -> Result<(), dbg::TraceEvent> {
        match addr {
            0 => {
                self.nrx0 = NRx0::from_bits_truncate(val);

                if !self.dac_on() {
                    self.enabled = false;
                }
            }
            1 => self.nrx1 = NRx1::from_bits_truncate(val),
            2 => self.nrx2 = NRx2::from_bits_truncate(val),
            3 => self.nrx3.0 = val,
            4 => self.write_to_nr4(val),
            _ => unreachable!(),
        };

        Ok(())
    }
}

pub struct APU {
    // Channels
    ch1: ToneChannel,
    ch2: ToneChannel,
    ch3: WaveChannel,

    // Channel 4 registers
    ch4_len_reg: IoReg<u8>,
    ch4_vol_reg: IoReg<u8>,
    ch4_cnt_reg: IoReg<u8>,
    ch4_ini_reg: IoReg<u8>,

    // Control registers
    nr50: NR50,
    nr51: NR51,
    nr52: NR52,

    // Audio sample channel
    sample_rate_counter: f32,
    sample_channel: Arc<ArrayQueue<i16>>,
    sample_period: f32,

    // Frame sequencer clocks
    clk_64: u32,
    clk_128: u32,
    clk_256: u32,
}

impl Default for APU {
    fn default() -> APU {
        APU {
            ch1: ToneChannel::new(
                NRx0::from_bits_truncate(0x80),
                NRx1::from_bits_truncate(0x8F),
                NRx2::from_bits_truncate(0xF3),
                IoReg(0x00),
                NRx4::from_bits_truncate(0xBF),
                true,
            ),

            ch2: ToneChannel::new(
                NRx0::from_bits_truncate(0xFF),
                NRx1::from_bits_truncate(0x3F),
                NRx2::from_bits_truncate(0x00),
                IoReg(0x00),
                NRx4::from_bits_truncate(0xBF),
                false,
            ),

            ch3: WaveChannel::default(),

            ch4_len_reg: IoReg(0xFF),
            ch4_vol_reg: IoReg(0x00),
            ch4_cnt_reg: IoReg(0x00),
            ch4_ini_reg: IoReg(0xBF),

            nr50: NR50::from_bits_truncate(0x77),
            nr51: NR51::from_bits_truncate(0xF3),
            nr52: NR52::from_bits_truncate(0xF1),

            // Create a sample channel that can hold up to 1024 samples.
            // At 44.1KHz, this is about 23ms worth of audio.
            sample_rate_counter: 0f32,
            sample_channel: Arc::new(ArrayQueue::new(1024)),
            sample_period: std::f32::INFINITY,

            // TODO according to [1] these clocks are slightly out of phase,
            // initialization and ticking should be fixed accordingly.
            // [1] http://gbdev.gg8.se/wiki/articles/Gameboy_sound_hardware#Frame_Sequencer
            clk_64: CLK_64_RELOAD,
            clk_128: CLK_128_RELOAD,
            clk_256: CLK_256_RELOAD,
        }
    }
}

impl APU {
    /// Instantiates a new APU producing samples at a frequency of `sample_rate`.
    pub fn new(sample_rate: f32) -> APU {
        let mut apu = APU::default();
        apu.set_sample_rate(sample_rate);
        apu
    }

    /// Advances the sound controller state machine by a single M-cycle.
    pub fn tick(&mut self) {
        self.clk_64 -= 4;
        self.clk_128 -= 4;
        self.clk_256 -= 4;

        // Internal timer clock tick
        self.ch1.tick();
        self.ch2.tick();
        self.ch3.tick();

        // Volume envelope clock tick
        if self.clk_64 == 0 {
            self.clk_64 = CLK_64_RELOAD;

            self.ch1.tick_vol_env();
            self.ch2.tick_vol_env();
        }

        // Sweep clock tick
        if self.clk_128 == 0 {
            self.clk_128 = CLK_128_RELOAD;

            self.ch1.tick_freq_sweep();
        }

        // Lenght counter clock tick
        if self.clk_256 == 0 {
            self.clk_256 = CLK_256_RELOAD;

            self.ch1.tick_len_ctr();
            self.ch2.tick_len_ctr();
            self.ch3.tick_len_ctr();
        }

        self.tick_mixer();
    }

    /// Update mixer output
    fn tick_mixer(&mut self) {
        self.sample_rate_counter += 4.0;

        // Update the audio channel
        if self.sample_rate_counter > self.sample_period {
            self.sample_rate_counter -= self.sample_period;

            let ch1 = self.ch1.get_channel_out();
            let ch2 = self.ch2.get_channel_out();
            let ch3 = self.ch3.get_channel_out();

            let mut so2 = 0;
            let mut so1 = 0;

            // If the peripheral is disabled, no sound is emitted.
            if !self.nr52.contains(NR52::PWR_CTRL) {
                self.sample_channel.push(0).unwrap_or(());
            } else {
                // Update LEFT speaker
                if self.nr51.contains(NR51::OUT1_L) {
                    so2 += ch1;
                }
                if self.nr51.contains(NR51::OUT2_L) {
                    so2 += ch2;
                }
                if self.nr51.contains(NR51::OUT3_L) {
                    so2 += ch3;
                }

                // Update RIGHT speaker
                if self.nr51.contains(NR51::OUT1_R) {
                    so1 += ch1;
                }
                if self.nr51.contains(NR51::OUT2_R) {
                    so1 += ch2;
                }
                if self.nr51.contains(NR51::OUT3_R) {
                    so1 += ch3;
                }

                // Adjust master volumes
                so2 *= 1 + i16::from((self.nr50 & NR50::LEFT_VOL).bits() >> 4);
                so1 *= 1 + i16::from((self.nr50 & NR50::RIGHT_VOL).bits());

                // Produce a sample which is an average of the two channels.
                // TODO implement true stero sound.
                self.sample_channel.push((so1 + so2) / 2).unwrap_or(());
            }
        }
    }

    /// Handles a read operation to the power register, mainly to read the sound register status.
    fn read_pwr_reg(&self) -> u8 {
        if !self.nr52.contains(NR52::PWR_CTRL) {
            0
        } else {
            let ch1_en = if self.ch1.enabled {
                NR52::OUT_1_EN
            } else {
                NR52::empty()
            };

            let ch2_en = if self.ch2.enabled {
                NR52::OUT_2_EN
            } else {
                NR52::empty()
            };

            let ch3_en = if self.ch3.enabled {
                NR52::OUT_3_EN
            } else {
                NR52::empty()
            };

            (self.nr52 | ch1_en | ch2_en | ch3_en).bits()
        }
    }

    /// Handles a write operation to NR52 aka the power control register
    fn write_to_pwr_reg(&mut self, val: u8) -> Result<(), dbg::TraceEvent> {
        let new_nr52 = NR52::from_bits_truncate(val) & NR52::PWR_CTRL;

        // When NR52 gets disabled, 0 is immediately written to all the other registers
        if !new_nr52.contains(NR52::PWR_CTRL) {
            for addr in 0xFF10..=0xFF25 {
                self.write(addr, 0)?;
            }
        }

        self.nr52 = new_nr52;

        Ok(())
    }

    /// Changes the current sample rate.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_period = (crate::CPU_CLOCK as f32) / sample_rate;
        self.sample_rate_counter = 0f32;
    }

    /// Returns a copy of the audio sample channel.
    pub fn get_sample_channel(&self) -> Arc<ArrayQueue<i16>> {
        self.sample_channel.clone()
    }
}

impl InterruptSource for APU {
    fn get_and_clear_irq(&mut self) -> Option<IrqSource> {
        None
    }
}

impl MemR for APU {
    fn read(&self, addr: u16) -> Result<u8, dbg::TraceEvent> {
        Ok(match addr {
            0xFF10..=0xFF14 => self.ch1.read(addr - 0xFF10)?,
            0xFF15..=0xFF19 => self.ch2.read(addr - 0xFF15)?,
            0xFF1A..=0xFF1E => self.ch3.read(addr - 0xFF1A)?,

            0xFF20 => self.ch4_len_reg.0 | 0xFF,
            0xFF21 => self.ch4_vol_reg.0,
            0xFF22 => self.ch4_cnt_reg.0,
            0xFF23 => self.ch4_ini_reg.0 | 0xBF,

            0xFF24 => self.nr50.bits(),
            0xFF25 => self.nr51.bits(),
            0xFF26 => self.read_pwr_reg() | 0x70,

            0xFF30..=0xFF3F => self.ch3.wave_ram[usize::from(addr) - 0xFF30],

            // Unused regs in this range: 0xFF15, 0xFF1F, 0xFF27..=0xFF2F
            _ => 0xFF,
        })
    }
}

impl MemW for APU {
    fn write(&mut self, addr: u16, val: u8) -> Result<(), dbg::TraceEvent> {
        // Writes to any register in range NR10-NR51 are ignored if the peripheral is off
        if addr < 0xFF26 && !self.nr52.contains(NR52::PWR_CTRL) {
            return Ok(());
        }

        match addr {
            0xFF10..=0xFF14 => self.ch1.write(addr - 0xFF10, val)?,
            0xFF15..=0xFF19 => self.ch2.write(addr - 0xFF15, val)?,
            0xFF1A..=0xFF1E => self.ch3.write(addr - 0xFF1A, val)?,

            0xFF20 => self.ch4_len_reg.0 = val,
            0xFF21 => self.ch4_vol_reg.0 = val,
            0xFF22 => self.ch4_cnt_reg.0 = val,
            0xFF23 => self.ch4_ini_reg.0 = val,

            0xFF24 => self.nr50 = NR50::from_bits_truncate(val),
            0xFF25 => self.nr51 = NR51::from_bits_truncate(val),
            0xFF26 => self.write_to_pwr_reg(val)?,

            0xFF30..=0xFF3F => self.ch3.wave_ram[usize::from(addr) - 0xFF30] = val,

            // Unused regs in this range: 0xFF15, 0xFF1F, 0xFF27..=0xFF2F
            _ => (),
        };

        Ok(())
    }
}
