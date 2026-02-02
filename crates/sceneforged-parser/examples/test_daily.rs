use sceneforged_parser::parse;

fn main() {
    let inputs = [
        "Series Title-0 (2010) - 1x05 - Missing Title",
        "20-1.2014.S02E01.720p.HDTV.x264-CROOKS",
        "App.Sonarr.Made.in.Canada.Part.Two.720p.HDTV.x264-2HD",
        "St_Series_209_Aids_And_Comfort",
        "this.is.a.show.2015.0308-yestv",
        "11-02 The Series Reaction (HD).m4v",
    ];

    for input in inputs {
        let result = parse(input);
        println!("Input: {}", input);
        println!("  Title: '{}'", *result.title);
        println!("  Seasons: {:?}", result.seasons.iter().map(|s| s.value).collect::<Vec<_>>());
        println!("  Episodes: {:?}", result.episodes.iter().map(|e| e.value).collect::<Vec<_>>());
        println!();
    }
}
