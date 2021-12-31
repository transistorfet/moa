
use std::iter::Iterator;

use crate::error::Error;
use crate::system::System;
use crate::memory::dump_slice;
use crate::signals::{EdgeSignal};
use crate::devices::{Clock, ClockElapsed, Address, Addressable, Steppable, Inspectable, Transmutable, read_beu16};
use crate::host::traits::{Host, BlitableSurface, HostData};
use crate::host::gfx::{Frame, FrameSwapper, MASK_COLOUR};


const REG_MODE_SET_1: usize             = 0x00;
const REG_MODE_SET_2: usize             = 0x01;
const REG_SCROLL_A_ADDR: usize          = 0x02;
const REG_WINDOW_ADDR: usize            = 0x03;
const REG_SCROLL_B_ADDR: usize          = 0x04;
const REG_SPRITES_ADDR: usize           = 0x05;
// Register 0x06 Unused
const REG_BACKGROUND: usize             = 0x07;
// Register 0x08 Unused
// Register 0x09 Unused
const REG_H_INTERRUPT: usize            = 0x0A;
const REG_MODE_SET_3: usize             = 0x0B;
const REG_MODE_SET_4: usize             = 0x0C;
const REG_HSCROLL_ADDR: usize           = 0x0D;
// Register 0x0E Unused
const REG_AUTO_INCREMENT: usize         = 0x0F;
const REG_SCROLL_SIZE: usize            = 0x10;
const REG_WINDOW_H_POS: usize           = 0x11;
const REG_WINDOW_V_POS: usize           = 0x12;
const REG_DMA_COUNTER_LOW: usize        = 0x13;
const REG_DMA_COUNTER_HIGH: usize       = 0x14;
const REG_DMA_ADDR_LOW: usize           = 0x15;
const REG_DMA_ADDR_MID: usize           = 0x16;
const REG_DMA_ADDR_HIGH: usize          = 0x17;


//const STATUS_PAL_MODE: u16              = 0x0001;
const STATUS_DMA_BUSY: u16              = 0x0002;
const STATUS_IN_HBLANK: u16             = 0x0004;
const STATUS_IN_VBLANK: u16             = 0x0008;
//const STATUS_ODD_FRAME: u16             = 0x0010;
//const STATUS_SPRITE_COLLISION: u16      = 0x0020;
//const STATUS_SPRITE_OVERFLOW: u16       = 0x0040;
//const STATUS_V_INTERRUPT: u16           = 0x0080;
//const STATUS_FIFO_FULL: u16             = 0x0100;
const STATUS_FIFO_EMPTY: u16            = 0x0200;

//const MODE1_BF_ENABLE_HV_COUNTER: u8    = 0x02;
const MODE1_BF_HSYNC_INTERRUPT: u8      = 0x10;

const MODE2_BF_V_CELL_MODE: u8          = 0x08;
const MODE2_BF_DMA_ENABLED: u8          = 0x10;
const MODE2_BF_VSYNC_INTERRUPT: u8      = 0x20;

const MODE3_BF_EXTERNAL_INTERRUPT: u8   = 0x08;
const MODE3_BF_V_SCROLL_MODE: u8        = 0x04;
const MODE3_BF_H_SCROLL_MODE: u8        = 0x03;

const MODE4_BF_H_CELL_MODE: u8          = 0x01;
const MODE4_BF_SHADOW_HIGHLIGHT: u8     = 0x08;



const DEV_NAME: &'static str = "ym7101";


#[derive(Copy, Clone, Debug, PartialEq)]
pub enum DmaType {
    None,
    Memory,
    Fill,
    Copy,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Memory {
    Vram,
    Cram,
    Vsram,
}

pub struct Ym7101Memory {
    pub vram: [u8; 0x10000],
    pub cram: [u8; 128],
    pub vsram: [u8; 80],

    pub transfer_type: u8,
    pub transfer_bits: u8,
    pub transfer_count: u32,
    pub transfer_remain: u32,
    pub transfer_src_addr: u32,
    pub transfer_dest_addr: u32,
    pub transfer_auto_inc: u32,
    pub transfer_fill_word: u16,
    pub transfer_run: DmaType,
    pub transfer_target: Memory,
    pub transfer_dma_busy: bool,

    pub ctrl_port_buffer: Option<u16>,
}

impl Ym7101Memory {
    pub fn new() -> Self {
        Self {
            vram: [0; 0x10000],
            cram: [0; 128],
            vsram: [0; 80],

            transfer_type: 0,
            transfer_bits: 0,
            transfer_count: 0,
            transfer_remain: 0,
            transfer_src_addr: 0,
            transfer_dest_addr: 0,
            transfer_auto_inc: 0,
            transfer_fill_word: 0,
            transfer_run: DmaType::None,
            transfer_target: Memory::Vram,
            transfer_dma_busy: false,

            ctrl_port_buffer: None,
        }
    }


    #[inline(always)]
    fn read_beu16(&self, target: Memory, addr: usize) -> u16 {
        let addr = match target {
            Memory::Vram => &self.vram[addr..],
            Memory::Cram => &self.cram[addr..],
            Memory::Vsram => &self.vsram[addr..],
        };
        read_beu16(addr)
    }

    #[inline(always)]
    fn read_u8(&self, target: Memory, addr: usize) -> u8 {
        match target {
            Memory::Vram => self.vram[addr],
            Memory::Cram => self.cram[addr],
            Memory::Vsram => self.vsram[addr],
        }
    }

    pub fn set_dma_mode(&mut self, mode: DmaType) {
        match mode {
            DmaType::None => {
                //self.status &= !STATUS_DMA_BUSY;
                self.transfer_dma_busy = false;
                self.transfer_run = DmaType::None;
            },
            _ => {
                //self.status |= STATUS_DMA_BUSY;
                self.transfer_dma_busy = true;
                self.transfer_run = mode;
            },
        }
    }

    pub fn setup_transfer(&mut self, upper: u16, lower: u16) {
        self.ctrl_port_buffer = None;
        self.transfer_type = ((((upper & 0xC000) >> 14) | ((lower & 0x00F0) >> 2))) as u8;
        self.transfer_dest_addr = ((upper & 0x3FFF) | ((lower & 0x0003) << 14)) as u32;
        self.transfer_target = match self.transfer_type & 0x0E {
            0 => Memory::Vram,
            4 => Memory::Vsram,
            _ => Memory::Cram,
        };
        info!("{}: transfer requested of type {:x} ({:?}) to address {:x}", DEV_NAME, self.transfer_type, self.transfer_target, self.transfer_dest_addr);
        if (self.transfer_type & 0x20) != 0 {
            if (self.transfer_type & 0x10) != 0 {
                self.set_dma_mode(DmaType::Copy);
            } else if (self.transfer_bits & 0x80) == 0 {
                self.set_dma_mode(DmaType::Memory);
            }
        }
    }

    pub fn get_transfer_target_mut(&mut self) -> (&mut [u8], usize) {
        match self.transfer_target {
            Memory::Vram => (&mut self.vram, 0x10000),
            Memory::Cram => (&mut self.cram, 128),
            Memory::Vsram => (&mut self.vsram, 80),
        }
    }

    pub fn read_data_port(&mut self, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        {
            let addr = self.transfer_dest_addr;
            let (target, length) = self.get_transfer_target_mut();
            for i in 0..data.len() {
                data[i] = target[(addr as usize + i) % length];
            }
        }
        self.transfer_dest_addr += self.transfer_auto_inc;
        debug!("{}: data port read {} bytes from {:?}:{:x} returning {:x},{:x}", DEV_NAME, data.len(), self.transfer_target, addr, data[0], data[1]);
        Ok(())
    }

    pub fn write_data_port(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        if (self.transfer_type & 0x30) == 0x20 {
            self.ctrl_port_buffer = None;
            self.transfer_fill_word = if data.len() >= 2 { read_beu16(data) } else { data[0] as u16 };
            self.set_dma_mode(DmaType::Fill);
        } else {
            debug!("{}: data port write {} bytes to {:?}:{:x} with {:?}", DEV_NAME, data.len(), self.transfer_target, self.transfer_dest_addr, data);

            {
                let addr = self.transfer_dest_addr as usize;
                let (target, length) = self.get_transfer_target_mut();
                for i in 0..data.len() {
                    target[(addr + i) % length] = data[i];
                }
            }
            self.transfer_dest_addr += self.transfer_auto_inc;
        }
        Ok(())
    }

    pub fn write_control_port(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        let value = read_beu16(data);
        match (data.len(), self.ctrl_port_buffer) {
            (2, None) => { self.ctrl_port_buffer = Some(value) },
            (2, Some(upper)) => self.setup_transfer(upper, read_beu16(data)),
            (4, None) => self.setup_transfer(value, read_beu16(&data[2..])),
            _ => { error!("{}: !!! error when writing to control port with {} bytes of {:?}", DEV_NAME, data.len(), data); },
        }
        Ok(())
    }

    pub fn step_dma(&mut self, system: &System) -> Result<(), Error> {
        if self.transfer_run != DmaType::None {
            // TODO we will just do the full dma transfer here, but it really should be stepped

            match self.transfer_run {
                DmaType::Memory => {
                    info!("{}: starting dma transfer {:x} from Mem:{:x} to {:?}:{:x} ({} bytes)", DEV_NAME, self.transfer_type, self.transfer_src_addr, self.transfer_target, self.transfer_dest_addr, self.transfer_remain);
                    let mut bus = system.get_bus();

                    while self.transfer_remain > 0 {
                        let mut data = [0; 2];
                        bus.read(self.transfer_src_addr as Address, &mut data)?;

                        {
                            let addr = self.transfer_dest_addr;
                            let (target, length) = self.get_transfer_target_mut();
                            target[(addr as usize) % length] = data[0];
                            target[(addr as usize + 1) % length] = data[1];
                        }

                        self.transfer_dest_addr += self.transfer_auto_inc;
                        self.transfer_src_addr += 2;
                        self.transfer_remain -= 1;
                    }
                },
                DmaType::Copy => {
                    info!("{}: starting dma copy from VRAM:{:x} to VRAM:{:x} ({} bytes)", DEV_NAME, self.transfer_src_addr, self.transfer_dest_addr, self.transfer_remain);
                    while self.transfer_remain > 0 {
                        self.vram[self.transfer_dest_addr as usize] = self.vram[self.transfer_src_addr as usize];
                        self.transfer_dest_addr += self.transfer_auto_inc;
                        self.transfer_src_addr += 1;
                        self.transfer_remain -= 1;
                    }
                },
                DmaType::Fill => {
                    info!("{}: starting dma fill to VRAM:{:x} ({} bytes) with {:x}", DEV_NAME, self.transfer_dest_addr, self.transfer_remain, self.transfer_fill_word);
                    while self.transfer_remain > 0 {
                        self.vram[self.transfer_dest_addr as usize] = self.transfer_fill_word as u8;
                        self.transfer_dest_addr += self.transfer_auto_inc;
                        self.transfer_remain -= 1;
                    }
                },
                _ => { warning!("{}: !!! error unexpected transfer mode {:x}", DEV_NAME, self.transfer_type); },
            }

            self.set_dma_mode(DmaType::None);
        }
        Ok(())
    }
}



#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ColourMode {
    Unmasked,
    Normal,
    Shadow,
    Highlight,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Scroll {
    ScrollA,
    ScrollB,
}

pub struct Ym7101State {
    pub status: u16,
    pub memory: Ym7101Memory,

    pub mode_1: u8,
    pub mode_2: u8,
    pub mode_3: u8,
    pub mode_4: u8,
    pub window_pos: (u8, u8),
    pub h_int_lines: u8,
    pub screen_size: (usize, usize),
    pub scroll_size: (usize, usize),
    pub window_offset: (usize, usize),
    pub background: u8,
    pub scroll_a_addr: usize,
    pub scroll_b_addr: usize,
    pub window_addr: usize,
    pub sprites_addr: usize,
    pub hscroll_addr: usize,

    pub last_clock: Clock,
    pub h_clock: u32,
    pub v_clock: u32,
    pub h_scanlines: u8,
}

impl Ym7101State {
    pub fn new() -> Self {
        Self {
            status: 0x3400 | STATUS_FIFO_EMPTY,
            memory: Ym7101Memory::new(),

            mode_1: 0,
            mode_2: 0,
            mode_3: 0,
            mode_4: 0,
            window_pos: (0, 0),
            h_int_lines: 0,
            screen_size: (0, 0),
            scroll_size: (0, 0),
            window_offset: (0, 0),
            background: 0,
            scroll_a_addr: 0,
            scroll_b_addr: 0,
            window_addr: 0,
            sprites_addr: 0,
            hscroll_addr: 0,

            last_clock: 0,
            h_clock: 0,
            v_clock: 0,
            h_scanlines: 0,
        }
    }

    pub fn update_screen_size(&mut self) {
        let h_cells = if (self.mode_4 & MODE4_BF_H_CELL_MODE) == 0 { 32 } else { 40 };
        let v_cells = if (self.mode_2 & MODE2_BF_V_CELL_MODE) == 0 { 28 } else { 30 };
        self.screen_size = (h_cells, v_cells);
    }

    pub fn update_window_offset(&mut self) {
        let win_h = ((self.window_pos.0 & 0x1F) << 1) as usize;
        let win_v = (self.window_pos.1 & 0x1F) as usize;
        let right = (self.window_pos.0 & 0x80) != 0;
        let down = (self.window_pos.1 & 0x80) != 0;

        self.window_offset = match (right, down) {
            (false, false) => (win_h, win_v),
            (true, false) => (win_h - self.screen_size.0, win_v),
            (false, true) => (win_h, win_v - self.screen_size.1),
            (true, true) => (win_h - self.screen_size.0, win_v - self.screen_size.1),
        }
    }

    #[inline(always)]
    fn hsync_int_enabled(&self) -> bool {
        (self.mode_1 & MODE1_BF_HSYNC_INTERRUPT) != 0
    }

    #[inline(always)]
    fn vsync_int_enabled(&self) -> bool {
        (self.mode_2 & MODE2_BF_VSYNC_INTERRUPT) != 0
    }

    #[inline(always)]
    fn external_int_enabled(&self) -> bool {
        (self.mode_3 & MODE3_BF_EXTERNAL_INTERRUPT) != 0
    }

    pub fn get_palette_colour(&self, palette: u8, colour: u8, mode: ColourMode) -> u32 {
        if colour == 0 && mode != ColourMode::Unmasked {
            return MASK_COLOUR;
        }
        let shift_enabled = (self.mode_4 & MODE4_BF_SHADOW_HIGHLIGHT) != 0;
        let rgb = self.memory.read_beu16(Memory::Cram, (((palette * 16) + colour) * 2) as usize);
        if !shift_enabled || mode == ColourMode::Normal {
            (((rgb & 0xF00) as u32) >> 4) | (((rgb & 0x0F0) as u32) << 8) | (((rgb & 0x00F) as u32) << 20)
        } else {
            let offset = if mode == ColourMode::Highlight { 0x808080 } else { 0x00 };
            (((rgb & 0xF00) as u32) >> 5) | (((rgb & 0x0F0) as u32) << 7) | (((rgb & 0x00F) as u32) << 19) | offset
        }
    }

    pub fn get_hscroll(&self, hcell: usize, line: usize) -> (usize, usize) {
        let scroll_addr = match self.mode_3 & MODE3_BF_H_SCROLL_MODE {
            0 => self.hscroll_addr,
            2 => self.hscroll_addr + (hcell << 5),
            3 => self.hscroll_addr + (hcell << 5) + (line * 2 * 2),
            _ => panic!("Unsupported horizontal scroll mode"),
        };

        let scroll_a = self.memory.read_beu16(Memory::Vram, scroll_addr) as usize & 0x3FF;
        let scroll_b = self.memory.read_beu16(Memory::Vram, scroll_addr + 2) as usize & 0x3FF;
        (scroll_a, scroll_b)
    }

    pub fn get_vscroll(&self, vcell: usize) -> (usize, usize) {
        let base_addr = if (self.mode_3 & MODE3_BF_V_SCROLL_MODE) == 0 {
            0
        } else {
            vcell >> 1
        };

        let scroll_a = self.memory.read_beu16(Memory::Vsram, base_addr) as usize & 0x3FF;
        let scroll_b = self.memory.read_beu16(Memory::Vsram, base_addr + 2) as usize & 0x3FF;
        (scroll_a, scroll_b)
    }

    #[inline(always)]
    pub fn get_pattern_addr(&self, cell_table: usize, cell_x: usize, cell_y: usize) -> usize {
        cell_table + ((cell_x + (cell_y * self.scroll_size.0 as usize)) << 1)
    }

    pub fn build_sprites_lists(&mut self) -> (Vec<Sprite>, Vec<Vec<usize>>) {
        let sprite_table = self.sprites_addr;
        let max_lines = self.screen_size.1 * 8;

        let mut sprites = vec![];
        let mut lines = vec![vec![]; max_lines];

        let mut link = 0;
        loop {
            let sprite = Sprite::new(&self.memory.vram[sprite_table + (link * 8)..]);

            let start_y = sprite.pos.1;
            for y in 0..(sprite.size.1 as i16 * 8) {
                let pos_y = start_y + y;
                if pos_y >= 0 && pos_y < max_lines as i16 {
                    lines[pos_y as usize].push(sprites.len());
                }
            }

            link = sprite.link as usize;
            sprites.push(sprite);

            if link == 0 {
                break;
            }
        }

        (sprites, lines)
    }

    pub fn get_pattern_pixel(&self, pattern_word: u16, x: usize, y: usize) -> u32 {
        let pattern_addr = (pattern_word & 0x07FF) << 5;
        let palette = ((pattern_word & 0x6000) >> 13) as u8;
        let h_rev = (pattern_word & 0x0800) != 0;
        let v_rev = (pattern_word & 0x1000) != 0;
        let mode = if (pattern_word & 0x8000) != 0 { ColourMode::Shadow } else { ColourMode::Normal };

        let offset = pattern_addr as usize + (if !v_rev { y } else { 7 - y }) as usize * 4 + (if !h_rev { x / 2 } else { 3 - (x / 2) }) as usize;
        let second = x % 2 == 1;
        let value = if (!h_rev && !second) || (h_rev && second) {
            self.get_palette_colour(palette, self.memory.vram[offset] >> 4, mode)
        } else {
            self.get_palette_colour(palette, self.memory.vram[offset] & 0x0f, mode)
        };

        value
    }

    pub fn draw_frame(&mut self, frame: &mut Frame) {
        let bg_colour = self.get_palette_colour((self.background & 0x30) >> 4, self.background & 0x0f, ColourMode::Unmasked);
        let (sprites, sprites_per_line) = self.build_sprites_lists();

        for y in 0..(self.screen_size.1 * 8) {
            let (hscrolling_a, hscrolling_b) = self.get_hscroll(y / 8, y % 8);

            for x in 0..(self.screen_size.0 * 8) {
                let (vscrolling_a, vscrolling_b) = self.get_vscroll(x / 8);

                let pixel_a_x = (x - hscrolling_a) % (self.scroll_size.0 * 8);
                let pixel_a_y = (y + vscrolling_a) % (self.scroll_size.1 * 8);
                let pattern_a_addr = self.get_pattern_addr(self.scroll_a_addr, pixel_a_x / 8, pixel_a_y / 8);
                let pattern_a_word = self.memory.read_beu16(Memory::Vram, pattern_a_addr);
                let priority_a = (pattern_a_word & 0x8000) != 0;
                let pixel_a = self.get_pattern_pixel(pattern_a_word, pixel_a_x % 8, pixel_a_y % 8);

                let pixel_b_x = (x - hscrolling_b) % (self.scroll_size.0 * 8);
                let pixel_b_y = (y + vscrolling_b) % (self.scroll_size.1 * 8);
                let pattern_b_addr = self.get_pattern_addr(self.scroll_b_addr, pixel_b_x / 8, pixel_b_y / 8);
                let pattern_b_word = self.memory.read_beu16(Memory::Vram, pattern_b_addr);
                let priority_b = (pattern_b_word & 0x8000) != 0;
                let pixel_b = self.get_pattern_pixel(pattern_b_word, pixel_b_x % 8, pixel_b_y % 8);

                let mut pixel_sprite = MASK_COLOUR;
                let mut priority_sprite = false;
                for sprite_num in sprites_per_line[y].iter() {
                    let offset_x = x as i16 - sprites[*sprite_num].pos.0;
                    let offset_y = y as i16 - sprites[*sprite_num].pos.1;

                    if offset_x >= 0 && offset_x < (sprites[*sprite_num].size.0 as i16 * 8) {
                        let pattern = sprites[*sprite_num].calculate_pattern(offset_x as usize / 8, offset_y as usize / 8);
                        priority_sprite = (pattern & 0x8000) != 0;

                        pixel_sprite = self.get_pattern_pixel(pattern, offset_x as usize % 8, offset_y as usize % 8);
                        if pixel_sprite != MASK_COLOUR {
                            break;
                        }
                    }
                }

                let pixels = match (priority_sprite, priority_a, priority_b) {
                    (false, false, true) =>
                        [ pixel_b, pixel_sprite, pixel_a, bg_colour, 0x000000 ],
                    (true, false, true) =>
                        [ pixel_sprite, pixel_b, pixel_a, bg_colour, 0x000000 ],
                    (false, true, false) =>
                        [ pixel_a, pixel_sprite, pixel_b, bg_colour, 0x000000 ],
                    (false, true, true) =>
                        [ pixel_a, pixel_b, pixel_sprite, bg_colour, 0x000000 ],
                    _ =>
                        [ pixel_sprite, pixel_a, pixel_b, bg_colour, 0x000000 ],
                };

                for pixel in pixels {
                    if pixel != MASK_COLOUR {
                        frame.set_pixel(x as u32, y as u32, pixel);
                        break;
                    }
                }
            }
        }
    }

    /*
    pub fn get_pattern_iter<'a>(&'a self, pattern_name: u16, line: i8) -> PatternIterator<'a> {
        let pattern_addr = (pattern_name & 0x07FF) << 5;
        let pattern_palette = ((pattern_name & 0x6000) >> 13) as u8;
        let h_rev = (pattern_name & 0x0800) != 0;
        let v_rev = (pattern_name & 0x1000) != 0;
        let mode = if (pattern_name & 0x8000) != 0 { ColourMode::Shadow } else { ColourMode::Normal };
        PatternIterator::new(&self, pattern_addr as u32, pattern_palette, mode, h_rev, v_rev, line)
    }

    pub fn get_scroll_a_pattern(&self, cell_x: usize, cell_y: usize, hscrolling_a: usize, vscrolling_a: usize) -> u16 {
        let pattern_x = ((cell_x + self.window_offset.0) as usize - (hscrolling_a / 8) as usize) % self.scroll_size.0 as usize;
        let pattern_y = ((cell_y + self.window_offset.1) as usize + (vscrolling_a / 8) as usize) % self.scroll_size.1 as usize;
        let pattern_addr = self.get_pattern_addr(self.scroll_a_addr, pattern_x, pattern_y);
        self.memory.read_beu16(Memory::Vram, pattern_addr)
    }

    pub fn get_scroll_b_pattern(&self, cell_x: usize, cell_y: usize, hscrolling_b: usize, vscrolling_b: usize) -> u16 {
        let pattern_x = ((cell_x + self.window_offset.0) as usize - (hscrolling_b / 8) as usize) % self.scroll_size.0 as usize;
        let pattern_y = ((cell_y + self.window_offset.1) as usize + (vscrolling_b / 8) as usize) % self.scroll_size.1 as usize;
        let pattern_addr = self.get_pattern_addr(self.scroll_b_addr, pattern_x, pattern_y);
        self.memory.read_beu16(Memory::Vram, pattern_addr)
    }


    pub fn draw_pattern(&mut self, frame: &mut Frame, pattern: u16, pixel_x: u32, pixel_y: u32) {
        let iter = self.get_pattern_iter(pattern, 0);
        frame.blit(pixel_x, pixel_y, iter, 8, 8);
    }

    pub fn draw_pattern_line(&mut self, frame: &mut Frame, pattern: u16, pixel_x: u32, pixel_y: u32, line: i8) {
        let iter = self.get_pattern_iter(pattern, line);
        frame.blit(pixel_x, pixel_y, iter, 8, 1);
    }



    pub fn draw_frame_old(&mut self, frame: &mut Frame) {
        self.draw_background(frame);
        self.draw_scrolls(frame);
        //self.draw_window(frame);
        self.draw_sprites(frame);
    }

    pub fn draw_background(&mut self, frame: &mut Frame) {
        let bg_colour = self.get_palette_colour((self.background & 0x30) >> 4, self.background & 0x0f, ColourMode::Normal);
        frame.clear(bg_colour);
    }

    #[inline(always)]
    pub fn draw_scrolls(&mut self, frame: &mut Frame) {
        if (self.mode_3 & MODE3_BF_H_SCROLL_MODE) != 3 {
            self.draw_scrolls_cell(frame);
        } else {
            self.draw_scrolls_line(frame);
        }
    }

    pub fn draw_scrolls_cell(&mut self, frame: &mut Frame) {
        let (cells_h, cells_v) = self.screen_size;

        for cell_y in 0..cells_v {
            let (hscrolling_a, hscrolling_b) = self.get_hscroll(cell_y, 0);
            for cell_x in 0..cells_h {
                let (vscrolling_a, vscrolling_b) = self.get_vscroll(cell_x);

                let pattern_b = self.get_scroll_b_pattern(cell_x, cell_y, hscrolling_b, vscrolling_b);
                let pattern_a = self.get_scroll_a_pattern(cell_x, cell_y, hscrolling_a, vscrolling_a);

                //if (pattern_b & 0x8000) != 0 && (pattern_a & 0x8000) == 0 {
                self.draw_pattern(frame, pattern_b, (cell_x << 3) as u32, (cell_y << 3) as u32 - (vscrolling_b as u32 % 8));
                self.draw_pattern(frame, pattern_a, (cell_x << 3) as u32, (cell_y << 3) as u32 - (vscrolling_b as u32 % 8));
            }
        }
    }

    pub fn draw_scrolls_line(&mut self, frame: &mut Frame) {
        let (cells_h, cells_v) = self.screen_size;

        for cell_y in 0..cells_v {
            for line in 0..8 {
                let (hscrolling_a, hscrolling_b) = self.get_hscroll(cell_y, line as usize);
                for cell_x in 0..cells_h {
                    let (_, vscrolling_b) = self.get_vscroll(cell_x);

                    let pattern_b = self.get_scroll_b_pattern(cell_x, cell_y, hscrolling_b, vscrolling_b);
                    self.draw_pattern_line(frame, pattern_b, (cell_x << 3) as u32 + (hscrolling_b as u32 % 8), (cell_y << 3) as u32 + line as u32 - (vscrolling_b as u32 % 8), line);
                }

                for cell_x in 0..cells_h {
                    let (vscrolling_a, _) = self.get_vscroll(cell_x);

                    let pattern_a = self.get_scroll_a_pattern(cell_x, cell_y, hscrolling_a, vscrolling_a);
                    self.draw_pattern_line(frame, pattern_a, (cell_x << 3) as u32 + (hscrolling_a as u32 % 8), (cell_y << 3) as u32 + line as u32 - (vscrolling_a as u32 % 8), line);
                }
            }
        }
    }

    pub fn draw_window(&mut self, frame: &mut Frame) {
        let cell_table = self.window_addr;
        let (cells_h, cells_v) = self.screen_size;

        // A window address of 0 disables the window
        if cell_table == 0 {
            return;
        }

        for cell_y in 0..cells_v {
            for cell_x in 0..cells_h {
                let pattern_w = self.memory.read_beu16(Memory::Vram, self.get_pattern_addr(cell_table, cell_x as usize, cell_y as usize));
                if pattern_w != 0 {
                    self.draw_pattern(frame, pattern_w, (cell_x << 3) as u32, (cell_y << 3) as u32);
                }
            }
        }
    }

    pub fn build_link_list(&mut self, sprite_table: usize, links: &mut [usize]) -> usize {
        links[0] = 0;
        let mut i = 0;
        loop {
            let link = self.memory.read_u8(Memory::Vram, sprite_table + (links[i] * 8) + 3);
            if link == 0 || link >= 80 {
                break;
            }
            i += 1;
            links[i] = link as usize;
        }
        i
    }

    pub fn draw_sprites(&mut self, frame: &mut Frame) {
        let sprite_table = self.sprites_addr;
        let (cells_h, cells_v) = self.screen_size;
        let (pos_limit_h, pos_limit_v) = (if cells_h == 32 { 383 } else { 447 }, if cells_v == 28 { 351 } else { 367 });

        let mut links = [0; 80];
        let lowest = self.build_link_list(sprite_table, &mut links);

        for i in (0..lowest + 1).rev() {
            let sprite_data = &self.memory.vram[(sprite_table + (links[i] * 8))..];

            let v_pos = read_beu16(&sprite_data[0..]);
            let size = sprite_data[2];
            let pattern_name = read_beu16(&sprite_data[4..]);
            let h_pos = read_beu16(&sprite_data[6..]);

            let (size_h, size_v) = (((size >> 2) & 0x03) as u16 + 1, (size & 0x03) as u16 + 1);
            let h_rev = (pattern_name & 0x0800) != 0;
            let v_rev = (pattern_name & 0x1000) != 0;

            for ih in 0..size_h {
                for iv in 0..size_v {
                    let (h, v) = (if !h_rev { ih } else { size_h - 1 - ih }, if !v_rev { iv } else { size_v - 1 - iv });
                    let (x, y) = (h_pos + ih * 8, v_pos + iv * 8);
                    if x > 128 && x < pos_limit_h && y > 128 && y < pos_limit_v {
                        let iter = self.get_pattern_iter(((pattern_name & 0x07FF) + (h * size_v) + v) | (pattern_name & 0xF800), 0);

                        frame.blit(x as u32 - 128, y as u32 - 128, iter, 8, 8);
                    }
                }
            }
        }
    }
    */
}

pub struct Sprite {
    pub pos: (i16, i16),
    pub size: (u16, u16),
    pub rev: (bool, bool),
    pub pattern: u16,
    pub link: u8,
}

impl Sprite {
    pub fn new(sprite_data: &[u8]) -> Self {
        let v_pos = read_beu16(&sprite_data[0..]);
        let size = sprite_data[2];
        let link = sprite_data[3];
        let pattern = read_beu16(&sprite_data[4..]);
        let h_pos = read_beu16(&sprite_data[6..]);

        let (size_h, size_v) = (((size >> 2) & 0x03) as u16 + 1, (size & 0x03) as u16 + 1);
        let h_rev = (pattern & 0x0800) != 0;
        let v_rev = (pattern & 0x1000) != 0;

        Self {
            pos: (h_pos as i16 - 128, v_pos as i16 - 128),
            size: (size_h, size_v),
            rev: (h_rev, v_rev),
            pattern,
            link,
        }
    }

    pub fn calculate_pattern(&self, cell_x: usize, cell_y: usize) -> u16 {
        let (h, v) = (if !self.rev.0 { cell_x } else { self.size.0 as usize - 1 - cell_x }, if !self.rev.1 { cell_y } else { self.size.1 as usize - 1 - cell_y });
        (self.pattern & 0xF800) | ((self.pattern & 0x07FF) + (h as u16 * self.size.1) + v as u16)
    }
}


/*
pub struct PatternIterator<'a> {
    state: &'a Ym7101State,
    palette: u8,
    mode: ColourMode,
    base: usize,
    h_rev: bool,
    v_rev: bool,
    line: i8,
    col: i8,
    second: bool,
}

impl<'a> PatternIterator<'a> {
    pub fn new(state: &'a Ym7101State, start: u32, palette: u8, mode: ColourMode, h_rev: bool, v_rev: bool, line: i8) -> Self {
        Self {
            state,
            palette,
            mode,
            base: start as usize,
            h_rev,
            v_rev,
            line,
            col: 0,
            second: false,
        }
    }
}

impl<'a> Iterator for PatternIterator<'a> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        let offset = self.base + (if !self.v_rev { self.line } else { 7 - self.line }) as usize * 4 + (if !self.h_rev { self.col } else { 3 - self.col }) as usize;
        let value = if (!self.h_rev && !self.second) || (self.h_rev && self.second) {
            self.state.get_palette_colour(self.palette, self.state.memory.vram[offset] >> 4, self.mode)
        } else {
            self.state.get_palette_colour(self.palette, self.state.memory.vram[offset] & 0x0f, self.mode)
        };

        if !self.second {
            self.second = true;
        } else {
            self.second = false;
            self.col += 1;
            if self.col >= 4 {
                self.col = 0;
                self.line += 1;
            }
        }

        Some(value)
    }
}
*/


pub struct Ym7101 {
    swapper: FrameSwapper,
    state: Ym7101State,

    pub external_interrupt: HostData<bool>,
    pub frame_complete: EdgeSignal,
}

impl Ym7101 {
    pub fn new<H: Host>(host: &mut H, external_interrupt: HostData<bool>) -> Ym7101 {
        let swapper = FrameSwapper::new(320, 224);

        host.add_window(FrameSwapper::to_boxed(swapper.clone())).unwrap();

        Ym7101 {
            swapper,
            state: Ym7101State::new(),
            external_interrupt,
            frame_complete: EdgeSignal::new(),
        }
    }

    fn set_register(&mut self, word: u16) {
        let reg = ((word & 0x1F00) >> 8) as usize;
        let data = (word & 0x00FF) as u8;
        info!("{}: register {:x} set to {:x}", DEV_NAME, reg, data);
        self.update_register_value(reg, data);
    }

    fn update_register_value(&mut self, reg: usize, data: u8) {
        match reg {
            REG_MODE_SET_1 => { self.state.mode_1 = data; },
            REG_MODE_SET_2 => {
                self.state.mode_2 = data;
                self.state.update_screen_size();
            },
            REG_SCROLL_A_ADDR => { self.state.scroll_a_addr = (data as usize) << 10; },
            REG_WINDOW_ADDR => { self.state.window_addr = (data as usize) << 10; },
            REG_SCROLL_B_ADDR => { self.state.scroll_b_addr = (data as usize) << 13; },
            REG_SPRITES_ADDR => { self.state.sprites_addr = (data as usize) << 9; },
            REG_BACKGROUND => { self.state.background = data; },
            REG_H_INTERRUPT => { self.state.h_int_lines = data; },
            REG_MODE_SET_3 => { self.state.mode_3 = data; },
            REG_MODE_SET_4 => {
                self.state.mode_4 = data;
                self.state.update_screen_size();
            },
            REG_HSCROLL_ADDR => { self.state.hscroll_addr = (data as usize) << 10; },
            REG_AUTO_INCREMENT => { self.state.memory.transfer_auto_inc = data as u32; },
            REG_SCROLL_SIZE => {
                let h = decode_scroll_size(data & 0x03);
                let v = decode_scroll_size((data >> 4) & 0x03);
                self.state.scroll_size = (h, v);
            },
            REG_WINDOW_H_POS => {
                self.state.window_pos.0 = data;
                self.state.update_window_offset();
            },
            REG_WINDOW_V_POS => {
                self.state.window_pos.1 = data;
                self.state.update_window_offset();
            },
            REG_DMA_COUNTER_LOW => {
                self.state.memory.transfer_count = (self.state.memory.transfer_count & 0xFF00) | data as u32;
                self.state.memory.transfer_remain = self.state.memory.transfer_count;
            },
            REG_DMA_COUNTER_HIGH => {
                self.state.memory.transfer_count = (self.state.memory.transfer_count & 0x00FF) | ((data as u32) << 8);
                self.state.memory.transfer_remain = self.state.memory.transfer_count;
            },
            REG_DMA_ADDR_LOW => {
                self.state.memory.transfer_src_addr = (self.state.memory.transfer_src_addr & 0xFFFE00) | ((data as u32) << 1);
            },
            REG_DMA_ADDR_MID => {
                self.state.memory.transfer_src_addr = (self.state.memory.transfer_src_addr & 0xFE01FF) | ((data as u32) << 9);
            },
            REG_DMA_ADDR_HIGH => {
                let mask = if (data & 0x80) == 0 { 0x7F } else { 0x3F };
                self.state.memory.transfer_bits = data & 0xC0;
                self.state.memory.transfer_src_addr = (self.state.memory.transfer_src_addr & 0x01FFFF) | (((data & mask) as u32) << 17);
            },
            0x6 | 0x8 | 0x9 | 0xE => { /* Reserved */ },
            _ => { panic!("{}: unknown register: {:?}", DEV_NAME, reg); },
        }
    }
}

fn decode_scroll_size(size: u8) -> usize {
    match size {
        0b00 => 32,
        0b01 => 64,
        0b11 => 128,
        _ => panic!("{}: invalid scroll size option {:x}", DEV_NAME, size),
    }
}

impl Steppable for Ym7101 {
    fn step(&mut self, system: &System) -> Result<ClockElapsed, Error> {
        let diff = (system.clock - self.state.last_clock) as u32;
        self.state.last_clock = system.clock;

        if self.state.external_int_enabled() && self.external_interrupt.get() {
            self.external_interrupt.set(false);
            system.get_interrupt_controller().set(true, 2, 26)?;
        }

        self.state.h_clock += diff;
        if (self.state.status & STATUS_IN_HBLANK) != 0 && self.state.h_clock >= 2_340 && self.state.h_clock <= 61_160 {
            self.state.status &= !STATUS_IN_HBLANK;
        }
        if (self.state.status & STATUS_IN_HBLANK) == 0 && self.state.h_clock >= 61_160 {
            self.state.status |= STATUS_IN_HBLANK;

            self.state.h_scanlines = self.state.h_scanlines.wrapping_sub(1);
            if self.state.hsync_int_enabled() && self.state.h_scanlines == 0  {
                self.state.h_scanlines = self.state.h_int_lines;
                system.get_interrupt_controller().set(true, 4, 28)?;
            }
        }
        if self.state.h_clock > 63_500 {
            self.state.h_clock -= 63_500;
        }

        self.state.v_clock += diff;
        if (self.state.status & STATUS_IN_VBLANK) != 0 && self.state.v_clock >= 1_205_992 && self.state.v_clock <= 15_424_008 {
            self.state.status &= !STATUS_IN_VBLANK;
        }
        if (self.state.status & STATUS_IN_VBLANK) == 0 && self.state.v_clock >= 15_424_008 {
            self.state.status |= STATUS_IN_VBLANK;

            if self.state.vsync_int_enabled() {
                system.get_interrupt_controller().set(true, 6, 30)?;
            }

            self.swapper.swap();
            let mut frame = self.swapper.current.lock().unwrap();
            self.state.draw_frame(&mut frame);

            self.frame_complete.signal();
        }
        if self.state.v_clock > 16_630_000 {
            self.state.v_clock -= 16_630_000;
        }

        if (self.state.mode_2 & MODE2_BF_DMA_ENABLED) != 0 {
            self.state.memory.step_dma(system)?;
            self.state.status = (self.state.status & !STATUS_DMA_BUSY) | (if self.state.memory.transfer_dma_busy { STATUS_DMA_BUSY } else { 0 });
        }

        Ok((1_000_000_000 / 13_423_294) * 4)
    }
}

impl Addressable for Ym7101 {
    fn len(&self) -> usize {
        0x20
    }

    fn read(&mut self, mut addr: Address, data: &mut [u8]) -> Result<(), Error> {
        match addr {
            // Read from Data Port
            0x00 | 0x02 => self.state.memory.read_data_port(addr, data)?,

            // Read from Control Port
            0x04 | 0x05 | 0x06 | 0x07 => {
                debug!("{}: read status byte {:x}", DEV_NAME, self.state.status);
                for i in 0..data.len() {
                    data[i] = if (addr % 2) == 0 {
                        (self.state.status >> 8) as u8
                    } else {
                        (self.state.status & 0x00FF) as u8
                    };
                    addr += 1;
                }
            },

            _ => { println!("{}: !!! unhandled read from {:x}", DEV_NAME, addr); },
        }
        Ok(())
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        match addr {
            // Write to Data Port
            0x00 | 0x02 => self.state.memory.write_data_port(addr, data)?,

            // Write to Control Port
            0x04 | 0x06 => {
                debug!("{}: write {} bytes to port {:x} with data {:?}", DEV_NAME, data.len(), addr, data);

                let value = read_beu16(data);
                if (value & 0xC000) == 0x8000 {
                    self.set_register(value);
                    if data.len() == 4 {
                        let value = read_beu16(&data[2..]);
                        if (value & 0xC000) != 0x8000 {
                            return Err(Error::new(&format!("{}: unexpected second byte {:x}", DEV_NAME, value)));
                        }
                        self.set_register(value);
                    }
                } else {
                    self.state.memory.write_control_port(addr, data)?;
                    self.state.status = (self.state.status & !STATUS_DMA_BUSY) | (if self.state.memory.transfer_dma_busy { STATUS_DMA_BUSY } else { 0 });
                }
            },

            _ => { warning!("{}: !!! unhandled write to {:x} with {:?}", DEV_NAME, addr, data); },
        }
        Ok(())
    }
}

impl Transmutable for Ym7101 {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }

    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }

    fn as_inspectable(&mut self) -> Option<&mut dyn Inspectable> {
        Some(self)
    }
}


impl Inspectable for Ym7101 {
    fn inspect(&mut self, _system: &System, args: &[&str]) -> Result<(), Error> {
        match args[0] {
            "" | "state" => {
                self.state.dump_state();
            },
            "vram" => {
                self.state.dump_vram();
            },
            "vsram" => {
                self.state.dump_vsram();
            },
            _ => { },
        }
        Ok(())
    }
}


impl Ym7101State {
    pub fn dump_state(&self) {
        println!("");
        println!("Mode1: {:#04x}", self.mode_1);
        println!("Mode2: {:#04x}", self.mode_2);
        println!("Mode3: {:#04x}", self.mode_3);
        println!("Mode4: {:#04x}", self.mode_4);
        println!("");
        println!("Scroll A : {:#06x}", self.scroll_a_addr);
        println!("Window   : {:#06x}", self.window_addr);
        println!("Scroll B : {:#06x}", self.scroll_b_addr);
        println!("HScroll  : {:#06x}", self.hscroll_addr);
        println!("Sprites  : {:#06x}", self.sprites_addr);
        println!("");
        println!("DMA type  : {:?}", self.memory.transfer_type);
        println!("DMA Source: {:#06x}", self.memory.transfer_src_addr);
        println!("DMA Dest  : {:#06x}", self.memory.transfer_dest_addr);
        println!("DMA Count : {:#06x}", self.memory.transfer_count);
        println!("Auto-Inc  : {:#06x}", self.memory.transfer_auto_inc);
    }

    pub fn dump_vram(&self) {
        dump_slice(&self.memory.vram, 65536);
    }

    pub fn dump_vsram(&self) {
        dump_slice(&self.memory.vsram, 80);
    }
}

