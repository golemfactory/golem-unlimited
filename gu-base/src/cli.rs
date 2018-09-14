use prettytable::format::{self, TableFormat};

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
