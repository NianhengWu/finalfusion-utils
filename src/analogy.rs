use std::collections::HashSet;
use std::io::BufRead;
use std::process;

use clap::{App, Arg, ArgMatches};
use finalfusion::similarity::Analogy;
use stdinout::{Input, OrExit};

use crate::io::{read_embeddings_view, EmbeddingFormat};
use crate::FinalfusionApp;

pub struct AnalogyApp {
    embeddings_filename: String,
    embedding_format: EmbeddingFormat,
    input_filename: Option<String>,
    excludes: [bool; 3],
    k: usize,
}

impl FinalfusionApp for AnalogyApp {
    fn app() -> App<'static, 'static> {
        App::new("analogy")
            .about("Find words that fit an analogy")
            .arg(
                Arg::with_name("format")
                    .short("f")
                    .value_name("FORMAT")
                    .takes_value(true)
                    .possible_values(&[
                        "fasttext",
                        "finalfusion",
                        "finalfusion_mmap",
                        "word2vec",
                        "text",
                        "textdims",
                    ])
                    .default_value("finalfusion"),
            )
            .arg(
                Arg::with_name("neighbors")
                    .short("k")
                    .value_name("K")
                    .help("Return K nearest neighbors")
                    .takes_value(true)
                    .default_value("10"),
            )
            .arg(
                Arg::with_name("EMBEDDINGS")
                    .help("Embeddings file")
                    .index(1)
                    .required(true),
            )
            .arg(
                Arg::with_name("include")
                    .long("include")
                    .help("Specify query parts that should be allowed as answers.")
                    .possible_values(&["a", "b", "c"])
                    .multiple(true)
                    .takes_value(true)
                    .max_values(3),
            )
            .arg(Arg::with_name("INPUT").help("Input words").index(2))
    }

    fn parse(matches: &ArgMatches) -> Self {
        let embeddings_filename = matches.value_of("EMBEDDINGS").unwrap().to_owned();

        let input_filename = matches.value_of("INPUT").map(ToOwned::to_owned);

        let embedding_format = matches
            .value_of("format")
            .map(|f| EmbeddingFormat::try_from(f).or_exit("Cannot parse embedding format", 1))
            .unwrap();

        let k = matches
            .value_of("neighbors")
            .map(|v| v.parse().or_exit("Cannot parse k", 1))
            .unwrap();
        let excludes = matches
            .values_of("include")
            .map(|v| {
                let set = v.collect::<HashSet<_>>();
                let exclude_a = !set.contains("a");
                let exclude_b = !set.contains("b");
                let exclude_c = !set.contains("c");
                [exclude_a, exclude_b, exclude_c]
            })
            .unwrap_or_else(|| [true, true, true]);

        AnalogyApp {
            embeddings_filename,
            embedding_format,
            input_filename,
            excludes,
            k,
        }
    }

    fn run(&self) {
        let embeddings = read_embeddings_view(&self.embeddings_filename, self.embedding_format)
            .or_exit("Cannot read embeddings", 1);
        let input = Input::from(self.input_filename.as_ref());
        let reader = input.buf_read().or_exit("Cannot open input for reading", 1);

        for line in reader.lines() {
            let line = line.or_exit("Cannot read line", 1).trim().to_owned();
            if line.is_empty() {
                continue;
            }

            let split_line: Vec<&str> = line.split_whitespace().collect();
            if split_line.len() != 3 {
                eprintln!("Query does not consist of three tokens: {}", line);
                process::exit(1);
            }

            let results = match embeddings.analogy_masked(
                [&split_line[0], &split_line[1], &split_line[2]],
                self.excludes,
                self.k,
            ) {
                Ok(results) => results,
                Err(success) => {
                    print_missing_tokens(&split_line, &success);
                    continue;
                }
            };

            for analogy in results {
                println!("{}\t{}", analogy.word, analogy.similarity);
            }
        }
    }
}

fn print_missing_tokens(tokens: &[&str], successful: &[bool]) {
    assert_eq!(tokens.len(), successful.len());

    let missing = tokens
        .iter()
        .zip(successful)
        .filter_map(|(&token, &success)| if !success { Some(token) } else { None })
        .collect::<Vec<_>>();

    eprintln!("Could not compute embedding(s) for: {}", missing.join(", "));
}
