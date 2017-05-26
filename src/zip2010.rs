//! Support for chunks based on 2010 census population data.

use csv;
#[cfg(test)]
use env_logger;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::default::Default;

use errors::*;

/// The length of a basic zip code, in digits.
const ZIP_CODE_LENGTH: usize = 5;

/// Classifies Zip codes into geochunks based on 2010 census population data.
pub struct Classifier {
    /// The approximate number of people we want to put in each chunk.
    target_population: u64,
    /// Map from zip code prefixes to chunk IDs.
    chunk_id_for_prefix: HashMap<String, String>,
}

impl Classifier {
    /// Create a new classifier, specifying how many people we'd ideally
    /// want to see in each chunk.
    pub fn new(target_population: u64) -> Classifier {
        let prefix_population = PrefixPopulation::new();
        let mut chunk_id_for_prefix = HashMap::<String, String>::new();
        prefix_population.build_chunks_recursive(target_population,
                                                 "",
                                                 &mut chunk_id_for_prefix);
        Classifier {
            target_population: target_population,
            chunk_id_for_prefix: chunk_id_for_prefix,
        }
    }

    /// Given a zip code, return the geochunk identifier.  Returns an error
    /// if the `zip` code is invalid.
    pub fn chunk_for(&self, zip: &str) -> Result<&str> {
        for i_rev in 0..(ZIP_CODE_LENGTH+1) {
            let i = ZIP_CODE_LENGTH - i_rev;
            if let Some(chunk_id) = self.chunk_id_for_prefix.get(&zip[..i]) {
                return Ok(chunk_id);
            }
        }
        Ok("")
    }
}

#[test]
fn classifies_sample_zip_codes_as_expected() {
    let _ = env_logger::init();
    let classifier = Classifier::new(250000);
    assert_eq!(classifier.chunk_for("01000").unwrap(), "010_0");
    assert_eq!(classifier.chunk_for("07720").unwrap(), "077_1");
}

type PrefixPopulationMaps = [HashMap<String, u64>; ZIP_CODE_LENGTH + 1];

/// Directly include our zip code population data in our application binary
/// for ease of distribution and packaging.
const ZIP_POPULATION_CSV: &'static str = include_str!("zip2010.csv");

/// The population associated with a zip code prefix.
struct PrefixPopulation {
    maps: PrefixPopulationMaps,
}

impl PrefixPopulation {
    fn new() -> PrefixPopulation {
        let mut maps = PrefixPopulationMaps::default();

        let mut rdr = csv::Reader::from_string(ZIP_POPULATION_CSV);
        for row in rdr.decode() {
            let (zip, pop): (String, u64) =
                row.expect("Invalid CSV data built into executable");

            // For each prefix of this zip code, increment the population of
            // that prefix.
            for prefix_len in 0..maps.len() {
                // This is a very long way of writing `(... ||= 0) += pop`.
                match maps[prefix_len].entry(zip[0..prefix_len].to_owned()) {
                    Entry::Vacant(vacant) => {
                        vacant.insert(pop);
                    }
                    Entry::Occupied(mut occupied) => {
                        *occupied.get_mut() += pop;
                    }
                }
            }
        }

        PrefixPopulation { maps: maps }
    }

    /// Look up the population of a zip code prefix.  Calling this function
    /// with invalid data will panic, since this is intended to be called using
    /// purely compile-time data.
    fn lookup(&self, prefix: &str) -> u64 {
        if prefix.len() > ZIP_CODE_LENGTH {
            panic!("Invalid zip code prefix: {}", prefix);
        }
        // Look up the prefix, and return 0 if it isn't in our map.
        self.maps[prefix.len()]
            .get(prefix)
            .cloned()
            .unwrap_or_default()
    }

    // Build zip code chunks based on population data.
    fn build_chunks_recursive(&self,
                              target_population: u64,
                              prefix: &str,
                              chunk_id_for_prefix: &mut HashMap<String, String>) {
        let prefix_pop = self.lookup(prefix);
        if prefix_pop <= target_population {
            // We're small enough to fill a chunk on our own.
            trace!("Mapping {} (pop {}) to {}", prefix, prefix_pop, prefix);
            chunk_id_for_prefix.insert(prefix.to_owned(), prefix.to_owned());
        } else {
            // Check each possible "child" of this prefix, recursing for any
            // that are greater than or equal to our target size.  Collect
            // the smaller children in `leftovers`.
            let mut leftovers = vec![];
            for digit in 0..10 {
                let child_prefix = format!("{}{}", prefix, digit);
                let child_pop = self.lookup(&child_prefix);
                if child_pop >= target_population {
                    self.build_chunks_recursive(target_population,
                                                &child_prefix,
                                                chunk_id_for_prefix);
                } else {
                    leftovers.push(child_prefix);
                }
            }

            // Group our leftovers into chunks with names like `{prefix}_{i}`.
            // It's important to include the zero-length chunks here, so that
            // post-2010 zip codes can be placed in some chunk.
            let mut chunk_idx: u64 = 0;
            let mut chunk_pop: u64 = 0;
            for child_prefix in leftovers {
                let child_pop = self.lookup(&child_prefix);
                assert!(child_pop < target_population);
                if chunk_pop + child_pop > target_population {
                    chunk_idx += 1;
                    chunk_pop = 0;
                }
                chunk_pop += child_pop;
                let chunk_id = format!("{}_{}", prefix, chunk_idx);
                trace!("Mapping {} (pop {}) to {}", child_prefix, child_pop, chunk_id);
                chunk_id_for_prefix.insert(child_prefix, chunk_id);
            }
        }
    }
}