use rodio::{Decoder, OutputStream, OutputStreamHandle, source::Source};
use std::sync::{Arc, Mutex};

use crossterm::terminal::{size, EndSynchronizedUpdate, BeginSynchronizedUpdate, EnterAlternateScreen, enable_raw_mode, disable_raw_mode, LeaveAlternateScreen, ClearType, Clear};
use crossterm::execute;
use crossterm::queue;
use crossterm::style::{Print, SetBackgroundColor, ResetColor, Color};

use std::time::Duration;
use std::thread;
use std::process::{Stdio, Command};
use std::io::{self, Write, Read, stdout, Cursor};
use std::{fs, env};

// Constants for parsing bitmap image headers
// For more information consult the bitmap format header specification
const HEADER_PIXEL_ARRAY_OFFSET:usize = 10;
const HEADER_WIDTH_OFFSET:usize = 18;
const HEADER_HEIGHT_OFFSET:usize = 22;
const HEADER_BITS_PER_PIXEL:usize = 28;

trait Method {
        fn get_pixel_array_offset(&self) -> usize;
	fn get_height(&self) -> u32;
	fn get_width(&self) -> u32;
	fn get_bits_per_pixel(&self) -> u8;
	fn print_row(&self, starting_pixel:usize);
	fn print_frame(&self) -> usize;
	fn get_end_of_next_frame(&self) -> usize;
	fn print_frame_from_index(&self, start:usize) -> usize;
}

impl Method for Vec<u8> {
	fn get_pixel_array_offset(&self) -> usize {
		return self[HEADER_PIXEL_ARRAY_OFFSET] as usize; 
	}
	fn get_width(&self) -> u32 {
		let byte_slice = &self[HEADER_WIDTH_OFFSET..HEADER_WIDTH_OFFSET+4];
		u32::from_le_bytes([byte_slice[0], byte_slice[1], byte_slice[2], byte_slice[3]])
	}
        fn get_height(&self) -> u32 {
		let byte_slice = &self[HEADER_HEIGHT_OFFSET..HEADER_HEIGHT_OFFSET+4];
		u32::from_le_bytes([byte_slice[0], byte_slice[1], byte_slice[2], byte_slice[3]])
        }
	fn get_bits_per_pixel(&self) -> u8 {
		return self[HEADER_BITS_PER_PIXEL];
	}
	fn print_row(&self, starting_pixel: usize) {
		let width = self.get_width();
		let bytes_per_pixel = self.get_bits_per_pixel() / 8;
		let mut next_pos = starting_pixel;
		for i in 0..width as usize {
			// RGB values are stored backwards i.e. BGR.
			queue!(io::stdout(),SetBackgroundColor(Color::Rgb {r: self[next_pos + 2], g: self[next_pos + 1], b: self[next_pos]}), Print(" "));
			next_pos += bytes_per_pixel as usize;
		}		
	}

	fn print_frame_from_index(&self, start:usize) -> usize{
                let bytes_per_pixel = (self.get_bits_per_pixel() / 8);
                let width = self.get_width() as usize;
                let height = self.get_height() as usize;
                //Each scan line is zero padded to the nearest 4-byte boundary.
                //If the image has a width that is not divisible by four, say, 21 bytes, there would be 3 bytes of padding at the end of every scan line.
                let step:usize = ((width * bytes_per_pixel as usize) + 3) & !3;

                let mut row_start = self.get_pixel_array_offset() + start as usize;
                let mut last_row:usize = row_start + (height * step) as usize;
		let end_of_frame = row_start + (height * step) as usize;

                // Scan lines are stored bottom to top instead of top to bottom.
                for i in 0..height {
                                last_row -= step;
                                self.print_row(last_row);
                }
		return end_of_frame;

	}



	fn print_frame(&self) -> usize {
	        let bytes_per_pixel = (self.get_bits_per_pixel() / 8);
	        let width = self.get_width() as usize;
	        let height = self.get_height() as usize;
		//Each scan line is zero padded to the nearest 4-byte boundary. 
		//If the image has a width that is not divisible by four, say, 21 bytes, there would be 3 bytes of padding at the end of every scan line.
	        let step:usize = ((width * bytes_per_pixel as usize) + 3) & !3;

	        let mut row_start = self.get_pixel_array_offset() as usize;
	        let mut last_row:usize = row_start + (height * step) as usize;
		let end_of_frame = row_start + (height * step) as usize;

		// Scan lines are stored bottom to top instead of top to bottom.
	        for i in 0..height {
				last_row -= step;
                		self.print_row(last_row);
        	}
		return end_of_frame;
	}
	fn get_end_of_next_frame(&self) -> usize {
                let bytes_per_pixel = (self.get_bits_per_pixel() / 8);
                let width = self.get_width() as usize;
                let height = self.get_height() as usize;
		let step:usize = ((width * bytes_per_pixel as usize) + 3) & !3;
		let mut row_start = self.get_pixel_array_offset() as usize;
		return row_start + (height * step) as usize;
	}
}

fn main() -> io::Result<()> {
	let args: Vec<String> = env::args().collect();
	if args.len() <= 1 {
		println!("No arguments provided. Please direct me to a file.");
		return Ok(());
	}
	let binding = args[1].to_string();
	let image_path = std::path::Path::new(&binding); // Store the first argument in the image_path variable
	
	// Convert input to bitmap
	let size = size().unwrap();
	let file = std::fs::File::open(image_path)?;
	let mut output = Command::new("ffmpeg")
        .arg("-i")
        .arg(image_path)
	.arg("-r") // Set the
	.arg("25")  // framerate
        .arg("-vf")
	.arg( format!("scale={}:{}", size.0 , size.1) )
        .arg("-f")
        .arg("image2pipe")
        .arg("-c:v")
        .arg("bmp")
        .arg("-")
	.stdout(Stdio::null()) // Redirect stdout to /dev/null
        .stderr(Stdio::null()) // Redirect stderr to /dev/null
        .stdout(Stdio::piped())
        .spawn()?
        .stdout
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Failed to capture standard output"))?;

	// Store ffmpeg's output in a vector of bytes
	let mut data:Vec<u8> = Vec::new();
	output.read_to_end(&mut data).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;


    // Spawn ffmpeg process to convert the input file to MP3 and capture stdout
	let mut ffmpeg_output = Command::new("ffmpeg")
        	.arg("-i")
        	.arg(image_path)
        	.arg("-f")
        	.arg("mp3")
        	.arg("pipe:1")  // Output to stdout
        	.stderr(Stdio::null())
        	.stdout(Stdio::piped())
        	.spawn()?;

	// Read stdout from ffmpeg process into a buffer
	let mut buffer = Vec::new();
	ffmpeg_output.stdout.take().unwrap().read_to_end(&mut buffer)?;

        // Initialize rodio audio output
	let (stream, stream_handle) = OutputStream::try_default().map_err(|e| {
		io::Error::new(io::ErrorKind::Other, format!("Failed to create OutputStream: {:?}", e))
	})?;
	// Create rodio sink
	let sink = rodio::Sink::try_new(&stream_handle).map_err(|e| {
		io::Error::new(io::ErrorKind::Other, format!("Failed to create Sink: {:?}", e))
	})?;

        execute!(io::stdout(), EnterAlternateScreen)?;
        enable_raw_mode().expect("Failed to enable raw mode");

	if !buffer.is_empty() {
    		// Create a cursor from the buffer to satisfy Decoder requirements
    		let cursor = Cursor::new(buffer);
		let source = Decoder::new(cursor).unwrap();
		// Play the audio
		sink.append(source);
		sink.play();
	}

	// Main loop to process data
	let mut next_frame:usize = 0;
	while next_frame < data.len() {
		// Create a shared flag to indicate whether the thread has finished sleeping
		let finished_flag = Arc::new(Mutex::new(false));

		// Spawn a new thread for handling the sleep synchronization
		let finished_flag_clone = Arc::clone(&finished_flag);
		thread::spawn(move || {
			// Simulate processing
			thread::sleep(Duration::from_millis(40));
			// Set the flag to true once the sleep is over
			let mut flag = finished_flag_clone.lock().unwrap();
			*flag = true;
		});

	        execute!(io::stdout(), BeginSynchronizedUpdate)?;
		next_frame = data.print_frame_from_index(next_frame);
		// Wait until the flag becomes true
		loop {
			let flag = finished_flag.lock().unwrap();
			if *flag {
				break; // Break the loop once the sleep is over
			}
			drop(flag); // Release the lock
			thread::sleep(Duration::from_micros(1)); // Sleep before checking again
		}
		execute!(io::stdout(), EndSynchronizedUpdate)?;
	}
	disable_raw_mode().expect("Failed to disable raw mode");
	execute!(io::stdout(), ResetColor, LeaveAlternateScreen);
	Ok(())
}
