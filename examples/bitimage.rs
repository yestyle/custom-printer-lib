use custom_printer::{BitImageMode, CustomPrinter, CutType, FeedUnit};

fn main() {
    // Replace /dev/null with actual device node when the printer is connected
    // e.g.: /dev/usb/lp0
    let mut printer = CustomPrinter::new("/dev/null").unwrap();
    printer
        .bit_image(
            "tests/data/Thermal_Test_Image.png",
            BitImageMode::Dots24DoubleDensity,
        )
        .unwrap()
        .print_and_feed_paper(FeedUnit::Lines, 10)
        .cut_paper(CutType::TotalCut)
        .run()
        .unwrap();
}
