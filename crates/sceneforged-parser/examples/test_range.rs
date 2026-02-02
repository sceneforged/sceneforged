use sceneforged_parser::parse;

fn main() {
    let inputs = [
        "Series.S03E01-06.DUAL.BDRip.XviD.AC3.-HELLYWOOD",
        "S02E03-04-05.720p.BluRay-FUTV",
        "Series.S05E01-02.720p.5.1Ch.BluRay",
        "Series.S05E01-E02.720p.5.1Ch.BluRay",
    ];

    for input in inputs {
        let result = parse(input);
        println!("Input: {}", input);
        println!(
            "  Episodes: {:?}",
            result.episodes.iter().map(|e| e.value).collect::<Vec<_>>()
        );
        println!("");
    }
}
