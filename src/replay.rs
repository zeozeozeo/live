use crate::utils::get_echo_base;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    number: i32,
    pressing_down: bool,
    is_player2: bool,
    ypos: f32,
    xpos: f32,
    rot: f32,
    yvel: f64,
    xvel: f64,
}

pub struct Replay;

impl Replay {
    pub fn echo_inputs() -> &'static mut [Frame] {
        let echo_base = get_echo_base().unwrap();
        let mut inputs = echo_base + 0x1507F4 - 4 - (12 * 6);
        unsafe {
            let start = (inputs as *mut usize).read();
            inputs += 4;
            let end = (inputs as *mut usize).read();
            // log::info!("start: {:x}, end: {:x}, len: {}", start, end, end - start);
            std::slice::from_raw_parts_mut(start as _, end - start)
        }
    }

    pub fn inputs_len() -> usize {
        Self::echo_inputs().len()
    }

    pub fn replay_pos() -> usize {
        let echo_base = get_echo_base().unwrap();
        let replay_pos = echo_base + 0x150448 + 4;
        unsafe { (replay_pos as *mut usize).read() }
    }

    pub fn search_action(frame: usize) -> Option<Frame> {
        let frame = frame as i32;
        let Ok(idx) = Self::echo_inputs().binary_search_by(|a| a.number.cmp(&frame)) else {
            return None;
        };
        if idx >= Self::inputs_len() {
            return None;
        }
        Some(Self::get_action(idx))
    }

    #[inline]
    pub fn get_action(index: usize) -> Frame {
        Self::echo_inputs()[index]
    }

    pub fn get_next_action() -> Frame {
        Self::get_action(Self::replay_pos())
    }

    pub fn next_frame(&mut self, delay: usize) -> Option<Frame> {
        let pos = Self::replay_pos();
        if pos < Self::inputs_len() {
            // still playing
        }
        None
    }
}
