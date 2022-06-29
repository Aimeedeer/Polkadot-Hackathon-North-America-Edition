#![cfg_attr(not(feature = "std"), no_std)]

use openbrush::traits::Balance;

pub fn min(x: Balance, y: Balance) -> Balance {
    if x < y {
        x
    } else {
        y
    }
}

pub fn sqrt(y: Balance) -> Balance {
    let mut z;
    if y > 3 {
        z = y;

        let mut x = y.checked_div(2).expect("error sqrt") + 1;
        while x < z {
            z = x;
            x = (y
                .checked_div(x)
                .expect("error sqrt")
                .checked_add(x)
                .expect("error sqrt"))
            .checked_div(2)
            .expect("error sqrt");
        }
    } else if y != 0 {
        z = 1;
    } else {
        z = 0; // todo: the right default value?
    }

    z
}
