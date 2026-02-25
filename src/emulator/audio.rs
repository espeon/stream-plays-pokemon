use ringbuf::{
    traits::{Consumer, Observer, Producer, Split},
    HeapRb,
};
use rustboyadvance_ng::prelude::AudioInterface;

/// GBA native sample rate.
pub const SAMPLE_RATE: i32 = 32768;

/// ~20ms worth of stereo i16 samples (interleaved left, right).
pub const CHUNK_SAMPLES: usize = 1310;

/// Thread-safe boxed audio interface (the emulator thread needs Send).
pub type SendAudioInterface = Box<dyn AudioInterface + Send>;

struct AudioCapture {
    producer: ringbuf::HeapProd<i16>,
}

// SAFETY: HeapProd<i16> is Send; AudioCapture owns it exclusively.
unsafe impl Send for AudioCapture {}

impl AudioInterface for AudioCapture {
    fn get_sample_rate(&self) -> i32 {
        SAMPLE_RATE
    }

    fn push_sample(&mut self, sample: &[i16; 2]) {
        let _ = self.producer.try_push(sample[0]);
        let _ = self.producer.try_push(sample[1]);
    }
}

pub struct AudioConsumer {
    pub consumer: ringbuf::HeapCons<i16>,
}

pub fn create_audio_pair(buffer_ms: u64) -> (SendAudioInterface, AudioConsumer) {
    let capacity = (SAMPLE_RATE as u64 * buffer_ms / 1000 * 2) as usize;
    let rb = HeapRb::<i16>::new(capacity.max(CHUNK_SAMPLES * 2 * 4));
    let (producer, consumer) = rb.split();
    (
        Box::new(AudioCapture { producer }),
        AudioConsumer { consumer },
    )
}

/// Drain one ~20ms chunk of audio from the consumer.
/// Returns None if not enough samples are buffered yet.
pub fn drain_chunk(consumer: &mut AudioConsumer) -> Option<Vec<u8>> {
    let needed = CHUNK_SAMPLES * 2; // stereo i16 values
    if consumer.consumer.occupied_len() < needed {
        return None;
    }
    let mut bytes = Vec::with_capacity(needed * 2);
    for _ in 0..needed {
        let sample = consumer.consumer.try_pop().unwrap_or(0);
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    Some(bytes)
}
