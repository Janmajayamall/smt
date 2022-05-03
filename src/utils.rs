use std::u8;

pub fn common_prefix(v1: &[u8], v2: &[u8]) -> usize {
    // println!("Common prefix for {:02x?} and {:02x?}", v1, v2);
    let mut count: usize = 0;
    for i in 0..(v1.len() * 8) {
        // println!(
        //     "Bit values at {} :: {:x?} and {:x?}",
        //     i,
        //     get_msb_at(v1, i),
        //     get_msb_at(v2, i)
        // );
        if get_msb_at(v1, i) != get_msb_at(v2, i) {
            break;
        }
        count += 1;
    }
    count
}

pub fn set_msb_at(data: &mut Vec<u8>, position: usize) {
    let index = position / 8;
    data[index] |= 1 << (7 - (position % 8));
}

pub fn get_msb_at(data: &[u8], position: usize) -> u8 {
    let index = position / 8;
    let p: u8 = u8::try_from(position % 8).expect("position should fit in u8");
    if data[index] & (1 << (7 - (p % 8))) > 0 {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::{common_prefix, get_msb_at, set_msb_at};

    #[test]
    fn test_common_prefix() {
        // 00010101010101010101010101000000
        let v1: [u8; 4] = [0x15, 0x55, 0x55, 0x40];
        // 00010101010101010001010101000000
        let v2: [u8; 4] = [0x15, 0x55, 0x15, 0x40];
        assert!(common_prefix(&v1, &v2) == 17);
    }

    #[test]
    fn test_msb_at() {
        // 00010101010101010101010101000000
        let v1: [u8; 4] = [0x15, 0x55, 0x55, 0x40];
        assert!(get_msb_at(&v1, 3) == 1);
        assert!(get_msb_at(&v1, 7) == 1);
        assert!(get_msb_at(&v1, 8) == 0);
    }

    #[test]
    fn test_set_msb_at() {
        // 00010101010101010001010101000000
        let mut v1: Vec<u8> = [0x15, 0x55, 0x15, 0x40].to_vec();

        set_msb_at(&mut v1, 17);
        assert!(v1 == [0x15, 0x55, 0x55, 0x40]);
    }
}
