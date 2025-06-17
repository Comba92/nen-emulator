use nom::bytes::complete::{tag, take_until, take_while, take_till, take_until1, take_while1, take_till1, is_a, is_not};
use nom::combinator::value;
use nom::multi::{separated_list0, separated_list1};
use nom::sequence::{separated_pair, delimited};
use nom::branch::{alt, permutation};
use nom::Parser;
use nom::IResult;

use nom::character::complete::i32;

type Coordinate = (i32, i32);

// fn delimited_whitespace<'a, 'b>(input: &'a str, token: &'b str) -> IResult<&'a str, &'a str> {
//   delimited(
//     take_while(char::is_whitespace),
//     tag(token),
//     take_while(char::is_whitespace)
//   ).parse(input)
// }

// fn delimited_whitespace(token: &str) -> impl Parser<&str, Error = nom::error::Error<&str>> {
//   delimited(
//     take_while(char::is_whitespace),
//     tag::<&str, &str, nom::error::Error<&str>>(token),
//     take_while(char::is_whitespace)
//   )
// }

fn delimited_whitespace<'a, E>(token: &'a str) -> impl Parser<&'a str, Error = E> 
where 
  E: nom::error::ParseError<&'a str>
{
  delimited(
    take_while(char::is_whitespace),
    tag(token),
    take_while(char::is_whitespace)
  )
}

fn parse_int_pair(input: &str) -> IResult<&str, (i32, i32)> {
  separated_pair(
    i32, 
    delimited_whitespace(","),
    i32
  ).parse(input)
}

fn parse_coordinate(input: &str) -> IResult<&str, Coordinate> {
  delimited(
    delimited_whitespace("("),
    take_until(")"),
    tag(")")
  )
  .and_then(parse_int_pair)
  .parse(input)
}

#[test]
fn cart_parsing() {
 
  let (rem, out) = parse_coordinate("  (  3  ,  5 )   ").unwrap();
  println!("{rem} :: {out:?}")
}

#[test]
fn parse_fds() {
  let rom = include_bytes!("../roms/Super Mario Bros. 2 (Japan) (En).fds");
  println!("{} {}", rom.len(), (rom.len() - 16) % 65500 == 0);

  let magic = &rom[..4];
  let sides_count = &rom[4];
  let zeroes = &rom[5..16];
  println!("{} {:?} {:?} {:?}", magic == [0x46, 0x44, 0x53, 0x1a], magic, sides_count, zeroes);

  let sides = rom.chunks(65500);
  println!("{}", sides.len());

  for side in sides {
    let block1 = side[0x00];
    dbg!(block1);
    let verify = str::from_utf8(&side[0x01..0x01 + 14]).unwrap_or_default();
    dbg!(verify);
    let licensee = side[0x0f];
    dbg!(licensee);
    let game_name = str::from_utf8(&side[0x10.. 0x10+3]).unwrap_or_default();
    dbg!(game_name);

    // todo: game type
    // todo: game version
    let side_number = side[0x16];
    dbg!(side_number);

    let disk_number = side[0x16];
    dbg!(disk_number);

    // todo: disk type 1

    let boot_file = side[0x19];
    dbg!(boot_file);
 
    // todo: manufacturing date

    let country = side[0x22];
    dbg!(country);

    // todo: rewrite date

    let actual_disk_side = side[0x35];
    dbg!(actual_disk_side);

    // todo: disk type 2
    // todo: disk version
    const first_block_len: usize = 0x38;

    let block2 = side[first_block_len];
    dbg!(block2);

    let files_count = side[first_block_len + 0x01];
    dbg!(files_count);

    let block3 = side[first_block_len + 0x02];
    dbg!(block3);

    let file_blocks = &side[first_block_len + 0x02..];
    dbg!(file_blocks.len());

    let mut file_iter = 0;
    const file_block_size: usize = 0x11;
    for i in 0..files_count as usize {
      println!();
      let file_number = file_blocks[file_iter + 0x01];
      dbg!(file_number);

      let file_id = file_blocks[file_iter + 0x02];
      dbg!(file_id);

      let file_name = &file_blocks[file_iter + 0x03 .. file_iter + 0x03 + 8];
      let name_utf8 = str::from_utf8(&file_name).unwrap_or_default();
      dbg!(name_utf8);

      let file_address = &file_blocks[file_iter + 0x0b .. file_iter + 0x0b + 2];
      let file_size = &file_blocks[file_iter + 0x0d .. file_iter + 0x0d + 2];
      let file_address = u16::from_le_bytes([file_address[0], file_address[1]]);
      let file_size = u16::from_le_bytes([file_size[0], file_size[1]]);
      dbg!(file_address);
      dbg!(file_size);

      let file_type = file_blocks[file_iter + 0x0f];
      dbg!(file_type);

      let block4 = file_blocks[file_iter + 0x10];
      dbg!(block4);

      file_iter += file_block_size + file_size as usize;
    }
  }
}