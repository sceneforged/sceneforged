use sceneforged_parser::parse;

fn main() {
    let input = "Series (2009) - [06x16] - Room 147.mp4";
    let result = parse(input);
    println!("Input: {}", input);
    println!("Title: '{}'", *result.title);
    println!("Year: {:?}", result.year.as_ref().map(|y| y.value));
    println!(
        "Seasons: {:?}",
        result.seasons.iter().map(|s| s.value).collect::<Vec<_>>()
    );
    println!(
        "Episodes: {:?}",
        result.episodes.iter().map(|e| e.value).collect::<Vec<_>>()
    );

    let input2 = "Series Title-0 (2010) - 1x05 - Missing Title";
    let result2 = parse(input2);
    println!("\nInput: {}", input2);
    println!("Title: '{}'", *result2.title);
}
