#![doc = include_str!("../README.md")]

use std::{
    fs::File,
    io::{self, Write},
};

// List of supported commands
// Printing commands
const PRINT: &[u8] = &[0x0A];
const PRINT_FEED_INCHES: &[u8] = &[0x1B, 0x4A];
const PRINT_FEED_LINES: &[u8] = &[0x1B, 0x64];
const SPEED_QUALITY: &[u8] = &[0x1B, 0x78];
const DENSITY: &[u8] = &[0x1D, 0x7C];
// Bit-image commands
const BIT_IMAGE: &[u8] = &[0x1B, 0x2A];
// Mechanism control commands
const TOTAL_CUT: &[u8] = &[0x1B, 0x69];
const PARTIAL_CUT: &[u8] = &[0x1B, 0x6D];

/// Modes supported by [`CustomPrinter::bit_image()`] function.
pub enum BitImageMode {
    /// 8 dot single density
    Dots8SingleDensity,
    /// 8 dot double density
    Dots8DoubleDensity,
    /// 24 dot single density
    Dots24SingleDensity,
    /// 24 dot double density
    Dots24DoubleDensity,
}

/// Cut types supported by [`CustomPrinter::cut_paper()`] function.
pub enum CutType {
    /// Total cut
    TotalCut,
    /// Partial cut, only valid for TL60 and TL80 printers.
    PartialCut,
}

/// Feed units supported by [`CustomPrinter::print_and_feed_paper()`] function.
pub enum FeedUnit {
    /// Feed the paper by number of vertical or horizontal motion unit inches
    Inches,
    /// Feed the paper by number of lines
    Lines,
}

/// Speeds supported by [`CustomPrinter::speed()`] function.
pub enum Speed {
    /// High speed (draft mode)
    High,
    /// Normal mode
    Normal,
    /// Low speed (high quality)
    Low,
}

/// Densities supported by [`CustomPrinter::density()`] function.
pub enum Density {
    /// -50%
    Minus50,
    /// -25%
    Minus25,
    /// 0%
    Zero,
    /// 25%
    Plus25,
    /// 50%
    Plus50,
}

/// The main struct to construct printing commands and accomplish actual printing.
///
/// The APIs are designed to be able to concatenate one after the other.
/// # Examples
///
/// ```no_run
/// # use custom_printer::{BitImageMode, CustomPrinter, CutType, FeedUnit};
/// let mut printer = CustomPrinter::new("/dev/usb/lp0").unwrap();
/// printer
///     .bit_image(
///         "logo.bmp",
///         BitImageMode::Dots24DoubleDensity
///     )
///     .unwrap()
///     .print()
///     .cut_paper(CutType::PartialCut)
///     .run()
///     .unwrap()
///     .bit_image(
///         "greeting.bmp",
///         BitImageMode::Dots24DoubleDensity
///     )
///     .unwrap()
///     .print_and_feed_paper(FeedUnit::Lines, 10)
///     .cut_paper(CutType::TotalCut)
///     .run()
///     .unwrap();
/// ```
pub struct CustomPrinter {
    file: File,
    cmd: Vec<u8>,
}

impl CustomPrinter {
    /// Create a new [`CustomPrinter`] with the device node `dev`.
    ///
    /// **NOTE:** Device node `dev` must be readable and writable by current user.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use custom_printer::CustomPrinter;
    /// CustomPrinter::new("/dev/usb/lp0")
    /// # ;
    /// ```
    pub fn new(dev: &str) -> Result<Self, io::Error> {
        let file = File::options().read(true).write(true).open(dev)?;
        Ok(Self {
            file,
            cmd: Vec::new(),
        })
    }

    pub(crate) fn convert_bitmap_to_bitimage(
        width: usize,
        height: usize,
        bitmap: &[u8],
        mode: &BitImageMode,
    ) -> Vec<u8> {
        // number of lines in a bank
        let bank = match mode {
            BitImageMode::Dots8SingleDensity | BitImageMode::Dots8DoubleDensity => 8,
            BitImageMode::Dots24SingleDensity | BitImageMode::Dots24DoubleDensity => 24,
        };
        // number of banks in bit image (might have padding lines in the last bank)
        let banks = (height + bank - 1) / bank;
        // number of bytes in bit image
        let size = banks * (bank / 8) * width;
        let mut bitimage = vec![0; size];
        // number of bytes in a line
        let step = width / 8;

        for i in 0..banks {
            for j in 0..width {
                for k in 0..bank {
                    let src = i * step * bank + k * step + j / 8;
                    let dst = i * width * (bank / 8) + j * (bank / 8) + k / 8;
                    if src < bitmap.len() && bitmap[src] & (0x80 >> (j % 8)) != 0 {
                        bitimage[dst] |= 0x80 >> (k % 8);
                    }
                }
            }
        }

        bitimage
    }

    /// Append commands for printing a bit image from `path` in `mode`. See [`BitImageMode`] for supported modes.
    ///
    /// **NOTE:** Because opening and reading the image file may fail, so the return Self is wrapped in a [`Result`]
    /// and needs to be unwrapped before concatenating with other constructing functions.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use custom_printer::{BitImageMode, CustomPrinter};
    /// # let mut printer = CustomPrinter::new("/dev/null").unwrap();
    /// printer
    ///     .bit_image(
    ///         "tests/data/Thermal_Test_Image.png",
    ///         BitImageMode::Dots24DoubleDensity
    ///     )
    ///     .unwrap();
    /// ```
    pub fn bit_image(&mut self, path: &str, mode: BitImageMode) -> Result<&mut Self, io::Error> {
        // Open image and convert to grayscale
        let img = image::open(path)
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?
            .grayscale();

        let width = img.width() as usize;
        let height = img.height() as usize;

        // convert 8bpp grayscaled image to 1 bpp bitmap
        let mut bitmap: Vec<u8> = vec![0; img.as_bytes().len() / 8];
        for (i, byte) in img.as_bytes().iter().enumerate() {
            // invert the bits
            if *byte == 0x00 {
                bitmap[i / 8] |= 0x80 >> (i % 8);
            }
        }

        // for (i, byte) in bitmap.iter().enumerate() {
        //     for j in 0..8 {
        //         print!("{}", if byte & (0x80 >> j) != 0 { 1 } else { 0 });
        //     }
        //     if i % (width / 8) == ((width / 8) - 1) {
        //         println!();
        //     }
        // }

        let bitimage = Self::convert_bitmap_to_bitimage(width, height, &bitmap, &mode);

        let (m, k) = match mode {
            BitImageMode::Dots8SingleDensity => (0x00, width),
            BitImageMode::Dots8DoubleDensity => (0x01, width),
            BitImageMode::Dots24SingleDensity => (0x20, width * 3),
            BitImageMode::Dots24DoubleDensity => (0x21, width * 3),
        };

        for i in 0..bitimage.len() / k {
            self.cmd.extend_from_slice(BIT_IMAGE);
            self.cmd
                .extend_from_slice(&[m, (width % 256) as u8, (width / 256) as u8]);
            self.cmd.extend_from_slice(&bitimage[i * k..(i + 1) * k]);
            // for j in 0..k {
            //     print!("{:02x} ", bitimage[i * k + j]);
            // }
            // println!();
        }

        Ok(self)
    }

    /// Append a command for cutting the paper totally ([`CutType::TotalCut`]) or partially ([`CutType::PartialCut`]).
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use custom_printer::{CustomPrinter, CutType};
    /// # let mut printer = CustomPrinter::new("/dev/null").unwrap();
    /// printer.cut_paper(CutType::TotalCut);
    /// ```
    pub fn cut_paper(&mut self, cut_type: CutType) -> &mut Self {
        match cut_type {
            CutType::TotalCut => {
                self.cmd.extend_from_slice(TOTAL_CUT);
            }
            CutType::PartialCut => {
                self.cmd.extend_from_slice(PARTIAL_CUT);
            }
        }

        self
    }

    /// Append a command for printing and line feeding.
    ///
    /// Either [`print()`](CustomPrinter::print()) or [`print_and_feed_paper()`](CustomPrinter::print_and_feed_paper()) should be appended
    /// before calling [`run()`](CustomPrinter::run()) to do actual printing.
    pub fn print(&mut self) -> &mut Self {
        self.cmd.extend_from_slice(PRINT);

        self
    }

    /// Append a command for printing and feeding the paper by `amount` of `unit`.
    ///
    /// Either [`print()`](CustomPrinter::print()) or [`print_and_feed_paper()`](CustomPrinter::print_and_feed_paper()) should be appended
    /// before calling [`run()`](CustomPrinter::run()) to do actual printing.
    pub fn print_and_feed_paper(&mut self, unit: FeedUnit, amount: u8) -> &mut Self {
        self.cmd.extend_from_slice(match unit {
            FeedUnit::Inches => PRINT_FEED_INCHES,
            FeedUnit::Lines => PRINT_FEED_LINES,
        });
        self.cmd.extend_from_slice(&[amount]);

        self
    }

    /// Append a command for selecting speed / quality mode.
    pub fn speed(&mut self, speed: &Speed) -> &mut Self {
        self.cmd.extend_from_slice(SPEED_QUALITY);
        self.cmd.extend_from_slice(&[match speed {
            Speed::High => 0,
            Speed::Normal => 1,
            Speed::Low => 2,
        }]);

        self
    }

    /// Append a command for setting printing density.
    pub fn density(&mut self, density: &Density) -> &mut Self {
        self.cmd.extend_from_slice(DENSITY);
        self.cmd.extend_from_slice(&[match density {
            Density::Minus50 => 0,
            Density::Minus25 => 1,
            Density::Zero => 2,
            Density::Plus25 => 3,
            Density::Plus50 => 4,
        }]);

        self
    }

    /// Run the constructed commands in the [`CustomPrinter`].
    ///
    /// The constructed commands will be cleared if the printing succeeds.
    ///
    /// **NOTE:** Because writing to the device node may fail, so the return Self is wrapped in a [`Result`]
    /// and needs to be unwrapped before concatenating with other constructing functions.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use custom_printer::{BitImageMode, CustomPrinter, CutType};
    /// # let mut printer = CustomPrinter::new("/dev/null").unwrap();
    /// printer
    ///     .bit_image(
    ///         "tests/data/Thermal_Test_Image.png",
    ///         BitImageMode::Dots24DoubleDensity
    ///     )
    ///     .unwrap()
    ///     .cut_paper(CutType::TotalCut)
    ///     .run()
    ///     .unwrap();
    /// ```
    pub fn run(&mut self) -> Result<&mut Self, io::Error> {
        self.file.write_all(&self.cmd)?;

        self.cmd.clear();
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const THERMAL_WIDTH: usize = 384;
    const THERMAL_HEIGHT: usize = 288;
    const THERMAL_TXT: &str = include_str!("../tests/data/thermal.txt");
    const THERMAL_8DOTS: &[u8] = include_bytes!("../tests/data/thermal.b8");
    const THERMAL_24DOTS: &[u8] = include_bytes!("../tests/data/thermal.b24");
    const THERMAL_PNG_PATH: &str = "tests/data/Thermal_Test_Image.png";
    const DEV_NULL: &str = "/dev/null";

    #[test]
    fn test_cut_paper() {
        let mut printer = CustomPrinter::new(DEV_NULL).unwrap();
        assert_eq!(printer.cut_paper(CutType::TotalCut).cmd, TOTAL_CUT);

        let mut printer = CustomPrinter::new(DEV_NULL).unwrap();
        assert_eq!(printer.cut_paper(CutType::PartialCut).cmd, PARTIAL_CUT);
    }

    #[test]
    #[ignore]
    fn helper_prepare_bitimage() {
        let converter = |text: &str, inverted: bool, output: &mut File, bank: usize| {
            let lines: Vec<&str> = text.trim().split('\n').collect();
            let width = lines[0].len();
            let banks = (lines.len() + (bank - 1)) / bank;

            for i in 0..banks {
                for j in 0..width {
                    let mut byte: u8 = 0;
                    for k in 0..bank {
                        let line_no = i * bank + k;
                        // padding lines are always 0
                        if line_no < lines.len() {
                            let b = lines[line_no].chars().nth(j).unwrap();
                            if inverted {
                                if b == '0' {
                                    byte |= 0x80 >> (k % 8);
                                }
                            } else {
                                if b == '1' {
                                    byte |= 0x80 >> (k % 8);
                                }
                            }
                        }
                        if k % 8 == 7 {
                            output.write(&[byte]).ok();
                            byte = 0;
                        }
                    }
                }
            }
        };

        let mut output = File::options()
            .create(true)
            .write(true)
            .truncate(true)
            .open("tests/data/thermal.b8")
            .unwrap();
        converter(THERMAL_TXT, true, &mut output, 8);

        let mut output = File::options()
            .create(true)
            .write(true)
            .truncate(true)
            .open("tests/data/thermal.b24")
            .unwrap();
        converter(THERMAL_TXT, true, &mut output, 24);
    }

    fn convert_text_to_bitmap(text: &str, inverted: bool) -> Vec<u8> {
        let mut data = Vec::new();

        text.trim().split('\n').for_each(|line| {
            if line.len() % 8 != 0 {
                eprintln!("Length of each line of text must be dividable by 8");
                return;
            }
            for i in (0..line.len()).step_by(8) {
                if let Ok(byte) = u8::from_str_radix(&line[i..i + 8], 2) {
                    data.extend_from_slice(&[if inverted { !byte } else { byte }]);
                } else {
                    eprintln!("text contains characters neither 0 nor 1");
                    return;
                }
            }
        });

        data
    }

    #[test]
    fn test_convert_bitmap_to_bitimage_8dots() {
        let bitmap = convert_text_to_bitmap(THERMAL_TXT, true);
        assert_eq!(
            &CustomPrinter::convert_bitmap_to_bitimage(
                THERMAL_WIDTH,
                THERMAL_HEIGHT,
                &bitmap,
                &BitImageMode::Dots8SingleDensity
            ),
            THERMAL_8DOTS
        );
        assert_eq!(
            &CustomPrinter::convert_bitmap_to_bitimage(
                THERMAL_WIDTH,
                THERMAL_HEIGHT,
                &bitmap,
                &BitImageMode::Dots8DoubleDensity
            ),
            THERMAL_8DOTS
        );
    }

    #[test]
    fn test_convert_bitmap_to_bitimage_24dots() {
        let bitmap = convert_text_to_bitmap(THERMAL_TXT, true);
        assert_eq!(
            &CustomPrinter::convert_bitmap_to_bitimage(
                THERMAL_WIDTH,
                THERMAL_HEIGHT,
                &bitmap,
                &BitImageMode::Dots24SingleDensity
            ),
            THERMAL_24DOTS
        );
        assert_eq!(
            &CustomPrinter::convert_bitmap_to_bitimage(
                THERMAL_WIDTH,
                THERMAL_HEIGHT,
                &bitmap,
                &BitImageMode::Dots24DoubleDensity
            ),
            THERMAL_24DOTS
        );
    }

    #[test]
    fn test_bit_image() {
        let mut printer = CustomPrinter::new(DEV_NULL).unwrap();
        printer
            .bit_image(THERMAL_PNG_PATH, BitImageMode::Dots8SingleDensity)
            .unwrap();

        let mut printer = CustomPrinter::new(DEV_NULL).unwrap();
        printer
            .bit_image(THERMAL_PNG_PATH, BitImageMode::Dots8DoubleDensity)
            .unwrap();

        let mut printer = CustomPrinter::new(DEV_NULL).unwrap();
        printer
            .bit_image(THERMAL_PNG_PATH, BitImageMode::Dots24SingleDensity)
            .unwrap();

        let mut printer = CustomPrinter::new(DEV_NULL).unwrap();
        printer
            .bit_image(THERMAL_PNG_PATH, BitImageMode::Dots24DoubleDensity)
            .unwrap();
    }

    #[test]
    fn test_multiple_run() {}
}
