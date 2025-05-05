use safe_transmute::TriviallyTransmutable;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct EnvInfo {
    pub block_time: u64,
    pub transferred_value: u64,
    pub balance: u64,
    pub caller_id: [u8; 32],
    pub entity_kind: u32,
}

unsafe impl TriviallyTransmutable for EnvInfo {}