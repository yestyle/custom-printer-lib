use custom_printer::{BitImageMode, CustomPrinter, CutType};

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
        .cut_paper(CutType::TotalCut)
        .run()
        .unwrap();
}
