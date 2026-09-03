//! HPACK, against the specification's own worked examples.
//!
//! Those examples are the reason this file exists rather than a round-trip
//! suite. A codec that agrees with itself proves nothing here: HPACK's whole
//! job is to agree with **somebody else's** encoder, one block at a time,
//! carrying state between them. So the assertions below are the exact bytes the
//! specification prints, and the exact table contents it says should exist
//! after each step.

use alo_net::h2::hpack::{self, Field, Table};

fn field(name: &str, value: &str) -> Field {
    Field::new(name, value)
}

fn bytes(hex: &str) -> Vec<u8> {
    hex.split_whitespace()
        .filter_map(|pair| u8::from_str_radix(pair, 16).ok())
        .collect()
}

// --- The specification's request examples, in sequence -----------------------

/// C.3: three requests on one connection, without Huffman. The point is the
/// third block, which is four bytes — because the first two taught the table
/// what it needed.
#[test]
fn three_requests_in_sequence_get_smaller_as_the_table_learns() {
    let mut table = Table::new(4096);

    let first = hpack::decode(
        &bytes("82 86 84 41 0f 77 77 77 2e 65 78 61 6d 70 6c 65 2e 63 6f 6d"),
        &mut table,
    )
    .unwrap_or_else(|why| panic!("the first block: {why}"));
    assert_eq!(
        first,
        vec![
            field(":method", "GET"),
            field(":scheme", "http"),
            field(":path", "/"),
            field(":authority", "www.example.com"),
        ]
    );
    assert_eq!(table.len(), 1, "the authority should have been remembered");
    assert_eq!(table.used(), 57);

    let second = hpack::decode(
        &bytes("82 86 84 be 58 08 6e 6f 2d 63 61 63 68 65"),
        &mut table,
    )
    .unwrap_or_else(|why| panic!("the second block: {why}"));
    assert_eq!(
        second,
        vec![
            field(":method", "GET"),
            field(":scheme", "http"),
            field(":path", "/"),
            field(":authority", "www.example.com"),
            field("cache-control", "no-cache"),
        ],
        "index 0xbe should have come from what the first block added"
    );
    assert_eq!(table.used(), 110);

    let third = hpack::decode(
        &bytes(
            "82 87 85 bf 40 0a 63 75 73 74 6f 6d 2d 6b 65 79 0c 63 75 73 74 6f 6d 2d 76 61 6c 75 65",
        ),
        &mut table,
    )
    .unwrap_or_else(|why| panic!("the third block: {why}"));
    assert_eq!(
        third,
        vec![
            field(":method", "GET"),
            field(":scheme", "https"),
            field(":path", "/index.html"),
            field(":authority", "www.example.com"),
            field("custom-key", "custom-value"),
        ]
    );
    assert_eq!(table.used(), 164);
}

/// C.4: the same three requests, Huffman-coded. Same headers, fewer bytes, and
/// a different code path through the string reader.
#[test]
fn the_same_three_requests_huffman_coded() {
    let mut table = Table::new(4096);

    let first = hpack::decode(
        &bytes("82 86 84 41 8c f1 e3 c2 e5 f2 3a 6b a0 ab 90 f4 ff"),
        &mut table,
    )
    .unwrap_or_else(|why| panic!("the first block: {why}"));
    assert_eq!(first.last(), Some(&field(":authority", "www.example.com")));
    assert_eq!(table.used(), 57);

    let second = hpack::decode(&bytes("82 86 84 be 58 86 a8 eb 10 64 9c bf"), &mut table)
        .unwrap_or_else(|why| panic!("the second block: {why}"));
    assert_eq!(second.last(), Some(&field("cache-control", "no-cache")));

    let third = hpack::decode(
        &bytes("82 87 85 bf 40 88 25 a8 49 e9 5b a9 7d 7f 89 25 a8 49 e9 5b b8 e8 b4 bf"),
        &mut table,
    )
    .unwrap_or_else(|why| panic!("the third block: {why}"));
    assert_eq!(third.last(), Some(&field("custom-key", "custom-value")));
    assert_eq!(table.used(), 164);
}

/// C.5: responses, with a table small enough that entries are evicted — which
/// is the part a decoder gets wrong and does not notice for a long time.
#[test]
fn responses_with_a_table_too_small_to_hold_them_all() {
    let mut table = Table::new(256);

    let first = hpack::decode(
        &bytes(
            "48 03 33 30 32 58 07 70 72 69 76 61 74 65 61 1d 4d 6f 6e 2c 20 32 31 20 4f 63 74 20 \
             32 30 31 33 20 32 30 3a 31 33 3a 32 31 20 47 4d 54 6e 17 68 74 74 70 73 3a 2f 2f 77 \
             77 77 2e 65 78 61 6d 70 6c 65 2e 63 6f 6d",
        ),
        &mut table,
    )
    .unwrap_or_else(|why| panic!("the first response: {why}"));
    assert_eq!(
        first,
        vec![
            field(":status", "302"),
            field("cache-control", "private"),
            field("date", "Mon, 21 Oct 2013 20:13:21 GMT"),
            field("location", "https://www.example.com"),
        ]
    );
    assert_eq!(table.used(), 222);

    let second = hpack::decode(&bytes("48 03 33 30 37 c1 c0 bf"), &mut table)
        .unwrap_or_else(|why| panic!("the second response: {why}"));
    assert_eq!(
        second,
        vec![
            field(":status", "307"),
            field("cache-control", "private"),
            field("date", "Mon, 21 Oct 2013 20:13:21 GMT"),
            field("location", "https://www.example.com"),
        ],
        "the second response is four bytes and every value came from the table"
    );
    assert_eq!(table.used(), 222, "something should have been evicted");
}

// --- What is refused ---------------------------------------------------------

/// Index zero is not an index — it is the value that means "a name follows",
/// and reading it as one is off-by-one into the static table.
#[test]
fn index_zero_is_refused_rather_than_read_as_the_first_entry() {
    let mut table = Table::new(4096);
    assert!(hpack::decode(&bytes("80"), &mut table).is_err());
}

#[test]
fn an_index_past_the_end_of_the_table_is_refused() {
    let mut table = Table::new(4096);
    let why = hpack::decode(&bytes("ff 00"), &mut table)
        .err()
        .map(|why| why.why)
        .unwrap_or_default();
    assert!(why.contains("not in the table"), "{why:?}");
}

/// Continuation bytes carry seven bits each and nothing says how many there
/// are, so an integer is a place a peer can send a hundred bytes meaning
/// "overflow".
#[test]
fn an_integer_that_never_ends_is_refused_rather_than_overflowing() {
    let mut table = Table::new(4096);
    let mut block = vec![0xff];
    block.extend(std::iter::repeat_n(0xffu8, 40));
    block.push(0x00);
    let why = hpack::decode(&block, &mut table)
        .err()
        .map(|why| why.why)
        .unwrap_or_default();
    assert!(
        why.contains("larger than this engine will read"),
        "a forty-byte integer was read: {why:?}"
    );
}

#[test]
fn a_string_longer_than_the_block_it_is_in_is_refused() {
    let mut table = Table::new(4096);
    // A literal with a new name, whose name says it is 100 bytes long.
    let why = hpack::decode(&bytes("40 64 61 62"), &mut table)
        .err()
        .map(|why| why.why)
        .unwrap_or_default();
    assert!(why.contains("longer than the block"), "{why:?}");
}

/// The peer names a size and may not then exceed it. That is not a negotiation,
/// it is a peer choosing how much memory this end spends.
#[test]
fn a_size_update_larger_than_was_agreed_is_refused() {
    let mut table = Table::new(4096);
    // `001` then a large integer: grow to 8192, when 4096 was agreed.
    let why = hpack::decode(&bytes("3f e1 3f"), &mut table)
        .err()
        .map(|why| why.why)
        .unwrap_or_default();
    assert!(why.contains("was agreed"), "{why:?}");
}

/// A size update may only appear before any header in a block.
#[test]
fn a_size_update_in_the_middle_of_a_block_is_refused() {
    let mut table = Table::new(4096);
    // An indexed field, then a size update.
    let why = hpack::decode(&bytes("82 20"), &mut table)
        .err()
        .map(|why| why.why)
        .unwrap_or_default();
    assert!(why.contains("middle of a block"), "{why:?}");
}

/// Shrinking to nothing is how a sender says "forget everything", and it has to
/// actually forget.
#[test]
fn shrinking_the_table_to_nothing_empties_it() {
    let mut table = Table::new(4096);
    let _ = hpack::decode(
        &bytes("41 0f 77 77 77 2e 65 78 61 6d 70 6c 65 2e 63 6f 6d"),
        &mut table,
    );
    assert_eq!(table.len(), 1);
    let _ = hpack::decode(&bytes("20"), &mut table).unwrap_or_else(|why| panic!("{why}"));
    assert!(
        table.is_empty(),
        "the table kept an entry it had no room for"
    );
    assert_eq!(table.used(), 0);
}

/// Every failure here kills the connection. The table carries state between
/// blocks, so a block nobody could decode leaves it in a condition nobody can
/// reason about — resetting one stream and carrying on is the tempting, wrong
/// answer.
#[test]
fn every_decoding_failure_is_fatal_to_the_connection() {
    let mut table = Table::new(4096);
    for block in ["80", "ff 00", "82 20", "3f e1 3f", "40 64 61 62"] {
        let mut fresh = Table::new(4096);
        let why = hpack::decode(&bytes(block), &mut fresh);
        let Err(broken) = why else {
            panic!("{block} was accepted");
        };
        assert!(broken.fatal, "{block} was survivable, and it must not be");
        assert_eq!(broken.error, alo_net::h2::ErrorCode::CompressionError);
    }
    let _ = &mut table;
}

// --- Encoding ----------------------------------------------------------------

/// What we encode, a fresh decoder must read — including the table state, which
/// is what makes the second block small.
#[test]
fn what_this_engine_encodes_it_can_read_back_with_the_table_it_built() {
    let mut writing = Table::new(4096);
    let mut reading = Table::new(4096);

    let request = vec![
        field(":method", "GET"),
        field(":scheme", "https"),
        field(":path", "/index.html"),
        field(":authority", "www.example.com"),
        field("user-agent", "alo browser"),
    ];
    let first = hpack::encode(&request, &mut writing);
    assert_eq!(
        hpack::decode(&first, &mut reading).unwrap_or_default(),
        request
    );

    // The second time, everything is in both tables and the block collapses.
    let second = hpack::encode(&request, &mut writing);
    assert!(
        second.len() < first.len() / 2,
        "the second block was {} bytes against {} — the table is not being used",
        second.len(),
        first.len()
    );
    assert_eq!(
        hpack::decode(&second, &mut reading).unwrap_or_default(),
        request
    );
}

/// Never-indexed means never put in a table — not by us, and not by anything
/// downstream. It is how a sender says "this is a secret", and a relay that
/// forgot it would compress somebody's token into a shared table.
#[test]
fn a_never_indexed_header_is_not_added_to_the_table_and_stays_marked() {
    let mut writing = Table::new(4096);
    let mut reading = Table::new(4096);
    let secret = Field {
        name: "authorization".to_owned(),
        value: "Bearer a-real-token".to_owned(),
        never_indexed: true,
    };
    let block = hpack::encode(std::slice::from_ref(&secret), &mut writing);
    assert!(
        writing.is_empty(),
        "a never-indexed header was put in the table"
    );

    let back = hpack::decode(&block, &mut reading).unwrap_or_default();
    assert_eq!(back, vec![secret], "the never-indexed flag did not survive");
    assert!(reading.is_empty());
}
