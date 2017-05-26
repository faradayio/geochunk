//! A module to hold `Error`, etc., types generated by `error-chain`.

use csv;
use std::io;

error_chain! {
    foreign_links {
        Csv(csv::Error);
        Io(io::Error);
    }

    errors {
    }
}