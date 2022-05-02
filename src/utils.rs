pub fn common_prefix(v1: &[u8], v2: &[u8]) -> usize {
    let mut count: usize = 0;
    while v1[count] == v2[count] {
        count += 1;
    }
    count
}

pub fn set_msb_at(data: &mut Vec<u8>, position: usize) {
    let index = position / 8;
    while data.len() != index + 1 {
        data.push(0);
    }
    data[index] |= 1 << ((position % 8) - 7);
}
