use bytes::{Buf, Bytes, BytesMut};
use tokio_util::codec::Decoder;

const JPEG_EOI_LENGTH: usize = 2;

#[derive(Default)]
pub(crate) struct JpegFrameDecoder;

impl Decoder for JpegFrameDecoder {
    type Item = Bytes;
    type Error = std::io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if let Some(idx) = find_first_jpeg_eoi(buf) {
            let image_buf = buf.copy_to_bytes(idx + JPEG_EOI_LENGTH);
            Ok(Some(image_buf))
        } else {
            Ok(None)
        }
    }
}

fn find_first_jpeg_eoi(bytes: &BytesMut) -> Option<usize> {
    if bytes.len() < JPEG_EOI_LENGTH {
        None
    } else {
        (0..bytes.len() - 1).find(|&i| bytes[i] == 0xFF && bytes[i + 1] == 0xD9)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn decoder_none() {
        let mut decoder = JpegFrameDecoder;

        let data = [0xFF, 0xD8, 1, 1, 1];
        let mut data = BytesMut::from(&data[..]);

        let res = decoder.decode(&mut data);
        assert!(res.unwrap().is_none());
    }

    #[test]
    fn decoder_one() {
        let mut decoder = JpegFrameDecoder;

        let data = [0xFF, 0xD8, 1, 1, 1, 0xFF, 0xD9];
        let mut data = BytesMut::from(&data[..]);

        let res = decoder.decode(&mut data);
        let res = res.unwrap().unwrap();
        assert_eq!(res, [0xFFu8, 0xD8, 1, 1, 1, 0xFF, 0xD9][..]);
    }

    #[test]
    fn decoder_multiple() {
        let mut decoder = JpegFrameDecoder;

        let data = [
            0xFF, 0xD8, 1, 1, 1, 0xFF, 0xD9, 0xFF, 0xD8, 2, 2, 2, 0xFF, 0xD9,
        ];
        let mut data = BytesMut::from(&data[..]);

        let res = decoder.decode(&mut data);
        let res = res.unwrap().unwrap();
        assert_eq!(res, [0xFFu8, 0xD8, 1, 1, 1, 0xFF, 0xD9][..]);

        let res = decoder.decode(&mut data);
        let res = res.unwrap().unwrap();
        assert_eq!(res, [0xFFu8, 0xD8, 2, 2, 2, 0xFF, 0xD9][..]);
    }

    #[test]
    fn find_first_jpeg_eoi_none() {
        let data = [0xFF, 0xD8, 1, 1, 1];
        let pos = find_first_jpeg_eoi(&BytesMut::from(&data[..]));
        assert_eq!(pos, None)
    }

    #[test]
    fn find_first_jpeg_eoi_single() {
        let data = [0xFF, 0xD8, 1, 1, 1, 0xFF, 0xD9];
        let pos = find_first_jpeg_eoi(&BytesMut::from(&data[..]));
        assert_eq!(pos, Some(5))
    }

    #[test]
    fn find_first_jpeg_eoi_multiple() {
        let data = [
            0xFF, 0xD8, 1, 1, 1, 0xFF, 0xD9, 0xFF, 0xD8, 2, 2, 2, 0xFF, 0xD9,
        ];
        let pos = find_first_jpeg_eoi(&BytesMut::from(&data[..]));
        assert_eq!(pos, Some(5))
    }
}
