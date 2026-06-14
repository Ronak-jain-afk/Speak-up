use std::sync::Arc;
use std::sync::Mutex;

use ringbuf::traits::{Producer as RingProducer, Split};
use ringbuf::{HeapCons, HeapRb};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub struct DeviceInfo {
    pub id: String,
    pub name: String,
}

pub struct AudioCapture {
    device: Option<cpal::Device>,
    stream: Option<cpal::Stream>,
    config: Option<cpal::StreamConfig>,
    consumer: Option<HeapCons<f32>>,
    rms_level: Arc<Mutex<f32>>,
    is_capturing: Arc<Mutex<bool>>,
}

impl AudioCapture {
    pub fn new() -> Self {
        Self {
            device: None,
            stream: None,
            config: None,
            consumer: None,
            rms_level: Arc::new(Mutex::new(0.0)),
            is_capturing: Arc::new(Mutex::new(false)),
        }
    }

    pub fn enumerate_devices() -> Vec<DeviceInfo> {
        let host = cpal::default_host();
        match host.input_devices() {
            Ok(devices) => devices
                .filter_map(|d| {
                    let name = d.name().ok()?;
                    let id = name.clone();
                    Some(DeviceInfo { id, name })
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn start(&mut self, device_id: &str) -> Result<(), AudioError> {
        let host = cpal::default_host();
        let device = if device_id.is_empty() {
            host.default_input_device()
                .ok_or(AudioError::DeviceNotFound("No default input device".into()))?
        } else {
            host.input_devices()
                .map_err(|e| AudioError::DeviceNotFound(e.to_string()))?
                .find(|d| d.name().map(|n| n == device_id).unwrap_or(false))
                .ok_or_else(|| AudioError::DeviceNotFound(device_id.to_string()))?
        };

        let supported =
            device.default_input_config().map_err(|e| AudioError::StreamError(e.to_string()))?;
        let config: cpal::StreamConfig = supported.into();

        let rb = HeapRb::<f32>::new(16384);
        let (mut producer, consumer) = rb.split();

        let rms_level = self.rms_level.clone();
        let _is_capturing = self.is_capturing.clone();

        let err_callback = move |err: cpal::StreamError| {
            tracing::error!("Audio stream error: {}", err);
        };

        let data_callback = move |data: &[f32], _: &cpal::InputCallbackInfo| {
            for &sample in data {
                let _ = producer.try_push(sample);
            }
            let sum_sq: f32 = data.iter().map(|s| s * s).sum();
            let rms = (sum_sq / data.len() as f32).sqrt();
            if let Ok(mut level) = rms_level.lock() {
                *level = rms;
            }
        };

        let stream = device
            .build_input_stream(&config, data_callback, err_callback, None)
            .map_err(|e| AudioError::StreamError(e.to_string()))?;

        stream.play().map_err(|e| AudioError::StreamError(e.to_string()))?;

        self.device = Some(device);
        self.config = Some(config);
        self.consumer = Some(consumer);
        self.stream = Some(stream);
        if let Ok(mut capturing) = self.is_capturing.lock() {
            *capturing = true;
        }

        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(stream) = self.stream.take() {
            drop(stream);
        }
        self.consumer = None;
        self.config = None;
        self.device = None;
        if let Ok(mut capturing) = self.is_capturing.lock() {
            *capturing = false;
        }
        if let Ok(mut level) = self.rms_level.lock() {
            *level = 0.0;
        }
    }

    pub fn current_level(&self) -> f32 {
        self.rms_level.lock().map(|l| *l).unwrap_or(0.0)
    }

    pub fn is_capturing(&self) -> bool {
        self.is_capturing.lock().map(|c| *c).unwrap_or(false)
    }

    pub fn take_consumer(&mut self) -> Option<HeapCons<f32>> {
        self.consumer.take()
    }

    pub fn sample_rate(&self) -> u32 {
        self.config.as_ref().map(|c| c.sample_rate.0).unwrap_or(16000)
    }

    pub fn channels(&self) -> u16 {
        self.config.as_ref().map(|c| c.channels).unwrap_or(1)
    }
}

impl Default for AudioCapture {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub enum AudioError {
    DeviceNotFound(String),
    StreamError(String),
    PermissionDenied,
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioError::DeviceNotFound(msg) => write!(f, "Device not found: {}", msg),
            AudioError::StreamError(msg) => write!(f, "Stream error: {}", msg),
            AudioError::PermissionDenied => write!(f, "Permission denied"),
        }
    }
}

impl std::error::Error for AudioError {}

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;
