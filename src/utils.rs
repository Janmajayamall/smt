pub fn common_prefix(v1: &[u8], v2: &[u8]) -> usize {
    let mut count: usize = 0;
    for i in 0..(v1.len() * 8) {
        if get_msb_at(v1, i) != get_msb_at(v2, i) {
            break;
        }
        count += 1;
    }
    count
}

pub fn set_msb_at(data: &mut Vec<u8>, position: usize) {
    let index = position / 8;
    while data.len() != index + 1 {
        data.push(0);
    }
    data[index] |= 1 << (7 - (position % 8));
}

pub fn get_msb_at(data: &[u8], position: usize) -> u8 {
    let index = position / 8;
    data[index] | (1 << (7 - (position % 8)))
}
