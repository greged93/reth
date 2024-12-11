use alloy_primitives::{B256, U256};

/// Reverses the ordering of bits in a [`B256`] type.
pub fn b256_reverse_bits(b256: B256) -> B256 {
    let mut b256 = b256.0;
    for byte in &mut b256 {
        *byte = byte.reverse_bits();
    }
    B256::from(b256)
}

/// Clear the last byte of a [`B256`] type.
pub fn b256_clear_last_byte(mut b256: B256) -> B256 {
    // set the largest byte to 0
    <B256 as AsMut<[u8; 32]>>::as_mut(&mut b256)[31] = 0;
    b256
}

/// Clear the first byte of a [`B256`] type.
pub fn b256_clear_first_byte(mut b256: B256) -> B256 {
    // set the smallest byte to 0
    <B256 as AsMut<[u8; 32]>>::as_mut(&mut b256)[0] = 0;
    b256
}

/// Clear the most significant byte of a [`U256`] type.
pub fn u256_clear_msb(mut balance: U256) -> U256 {
    // set the most significant 8 bits to 0
    unsafe {
        balance.as_limbs_mut()[3] &= 0x00FFFFFFFFFFFFFF;
    }
    balance
}
