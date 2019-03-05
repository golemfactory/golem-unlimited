use output::{listing_format, ListingFormat};
use prettytable::{
    format::{self, TableFormat},
    row::Row,
    Table,
};

lazy_static! {
    pub static ref FORMAT_BASIC: TableFormat = format::FormatBuilder::new()
        .column_separator('│')
        .borders('│')
        .separators(
            &[format::LinePosition::Top],
            format::LineSeparator::new('─', '┬', '┌', '┐')
        )
        .separators(
            &[format::LinePosition::Intern],
            format::LineSeparator::new('─', '┼', '├', '┤')
        )
        .separators(
            &[format::LinePosition::Bottom],
            format::LineSeparator::new('─', '┴', '└', '┘')
        )
        .padding(1, 1)
        .build();
}

fn format_json<'a, RowIter, MsgFn>(title: Row, _empty_msg: MsgFn, it: RowIter)
where
    RowIter: Iterator<Item = Row>,
    MsgFn: FnOnce() -> &'a str,
{
    use std::collections::hash_map::RandomState;
    use std::collections::HashMap;
    use std::iter::FromIterator;
    use std::iter::IntoIterator;

    let mut vec = Vec::new();
    for row in it {
        let row = row.into_iter();
        let title = title.clone();
        let pairs = title.into_iter().zip(row.into_iter());
        let pairs = pairs.map(|x| (x.0.to_string(), x.1.to_string()));
        let map = HashMap::<_, _, RandomState>::from_iter(pairs);
        vec.push(map);
    }


    println!("{}", serde_json::to_string_pretty(&vec).unwrap());
}

fn format_pretty<'a, RowIter, MsgFn>(title: Row, empty_msg: MsgFn, it: RowIter)
where
    RowIter: Iterator<Item = Row>,
    MsgFn: FnOnce() -> &'a str,
{
    let mut table = Table::new();
    table.set_titles(title);
    let mut show = false;
    for row in it {
        table.add_row(row);
        show = true;
    }

    if show {
        table.set_format(*FORMAT_BASIC);
        table.printstd()
    } else {
        eprintln!("{}", empty_msg())
    }
}

pub fn format_table<'a, RowIter, MsgFn>(title: Row, empty_msg: MsgFn, it: RowIter)
where
    RowIter: Iterator<Item = Row>,
    MsgFn: FnOnce() -> &'a str,
{
    let function = match listing_format() {
        ListingFormat::Json => format_json,
        ListingFormat::Table => format_pretty,
    };

    function(title, empty_msg, it);
}
