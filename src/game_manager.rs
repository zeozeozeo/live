use crate::hooks::get_base;

pub struct GameManager {
    addr: usize,
}

impl GameManager {
    pub fn shared() -> Self {
        unsafe {
            Self {
                addr: (std::mem::transmute::<usize, unsafe extern "stdcall" fn() -> usize>(
                    get_base() + 0x121540,
                ))(),
            }
        }
    }
}
