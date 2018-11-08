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
        ).separators(
            &[format::LinePosition::Intern],
            format::LineSeparator::new('─', '┼', '├', '┤')
        ).separators(
            &[format::LinePosition::Bottom],
            format::LineSeparator::new('─', '┴', '└', '┘')
        ).padding(1, 1)
        .build();
}

pub fn format_table<'a, RowIter, MsgFn>(title: Row, empty_msg: MsgFn, it: RowIter)
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
