use eframe::egui;
use image::{AnimationDecoder, DynamicImage, GenericImageView};
use std::path::Path;
use std::time::{Duration, Instant};

/// Represents a single frame of an avatar (static or animated)
struct AvatarFrame {
    texture: egui::TextureHandle,
    duration: Duration, // How long to display this frame (0 for static images)
}

/// Avatar that can be either static or animated
pub struct Avatar {
    frames: Vec<AvatarFrame>,
    current_frame: usize,
    last_frame_time: Instant,
    is_animated: bool,
}

impl Avatar {
    /// Load an avatar from a file path (PNG, JPG, or GIF)
    pub fn load(ctx: &egui::Context, path: &str) -> Result<Self, String> {
        let path_obj = Path::new(path);

        if !path_obj.exists() {
            return Err(format!("Avatar file not found: {}", path));
        }

        let extension = path_obj.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match extension.as_str() {
            "gif" => Self::load_animated_gif(ctx, path),
            "png" | "jpg" | "jpeg" => Self::load_static(ctx, path),
            _ => Err(format!("Unsupported avatar format: {}", extension)),
        }
    }

    /// Load a static image (PNG/JPG)
    fn load_static(ctx: &egui::Context, path: &str) -> Result<Self, String> {
        let img = image::open(path)
            .map_err(|e| format!("Failed to load image {}: {}", path, e))?;

        let size = [img.width() as usize, img.height() as usize];
        let pixels = img.to_rgba8();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_raw());

        let texture = ctx.load_texture(
            path,
            color_image,
            egui::TextureOptions::LINEAR,
        );

        Ok(Self {
            frames: vec![AvatarFrame {
                texture,
                duration: Duration::ZERO, // Static images have no duration
            }],
            current_frame: 0,
            last_frame_time: Instant::now(),
            is_animated: false,
        })
    }

    /// Load an animated GIF
    fn load_animated_gif(ctx: &egui::Context, path: &str) -> Result<Self, String> {
        let file = std::fs::File::open(path)
            .map_err(|e| format!("Failed to open GIF {}: {}", path, e))?;

        let reader = std::io::BufReader::new(file);

        let decoder = image::codecs::gif::GifDecoder::new(reader)
            .map_err(|e| format!("Failed to decode GIF {}: {}", path, e))?;

        let frames_iter = decoder.into_frames();

        let mut avatar_frames = Vec::new();

        for (index, frame_result) in frames_iter.enumerate() {
            let frame = frame_result
                .map_err(|e| format!("Failed to decode GIF frame {}: {}", index, e))?;

            let delay = frame.delay();
            let duration = Duration::from_millis(
                (delay.numer_denom_ms().0 as u64 * 1000) / delay.numer_denom_ms().1 as u64
            );

            let buffer = frame.buffer();
            let size = [buffer.width() as usize, buffer.height() as usize];

            // Convert to RGBA
            let rgba_data: Vec<u8> = buffer.pixels()
                .flat_map(|p| {
                    let rgba = p.0;
                    [rgba[0], rgba[1], rgba[2], rgba[3]]
                })
                .collect();

            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba_data);

            let texture = ctx.load_texture(
                format!("{}_{}", path, index),
                color_image,
                egui::TextureOptions::LINEAR,
            );

            avatar_frames.push(AvatarFrame {
                texture,
                duration: if duration.as_millis() == 0 {
                    Duration::from_millis(100) // Default to 100ms if no delay specified
                } else {
                    duration
                },
            });
        }

        if avatar_frames.is_empty() {
            return Err(format!("GIF has no frames: {}", path));
        }

        Ok(Self {
            frames: avatar_frames,
            current_frame: 0,
            last_frame_time: Instant::now(),
            is_animated: true,
        })
    }

    /// Update animation state and advance to next frame if needed
    pub fn update(&mut self) {
        if !self.is_animated || self.frames.len() <= 1 {
            return;
        }

        let current_frame_duration = self.frames[self.current_frame].duration;
        let elapsed = self.last_frame_time.elapsed();

        if elapsed >= current_frame_duration {
            self.current_frame = (self.current_frame + 1) % self.frames.len();
            self.last_frame_time = Instant::now();
        }
    }

    /// Get the current frame's texture
    pub fn current_texture(&self) -> &egui::TextureHandle {
        &self.frames[self.current_frame].texture
    }

    /// Check if this avatar is animated
    pub fn is_animated(&self) -> bool {
        self.is_animated
    }

    /// Reset animation to first frame
    pub fn reset(&mut self) {
        self.current_frame = 0;
        self.last_frame_time = Instant::now();
    }
}

/// Container for all avatar states
pub struct AvatarSet {
    pub idle: Option<Avatar>,
    pub thinking: Option<Avatar>,
    pub active: Option<Avatar>,
}

impl AvatarSet {
    /// Load avatars from config paths
    pub fn load(
        ctx: &egui::Context,
        idle_path: Option<&str>,
        thinking_path: Option<&str>,
        active_path: Option<&str>,
    ) -> Self {
        let idle = idle_path.and_then(|path| {
            match Avatar::load(ctx, path) {
                Ok(avatar) => Some(avatar),
                Err(e) => {
                    tracing::warn!("Failed to load idle avatar: {}", e);
                    None
                }
            }
        });

        let thinking = thinking_path.and_then(|path| {
            match Avatar::load(ctx, path) {
                Ok(avatar) => Some(avatar),
                Err(e) => {
                    tracing::warn!("Failed to load thinking avatar: {}", e);
                    None
                }
            }
        });

        let active = active_path.and_then(|path| {
            match Avatar::load(ctx, path) {
                Ok(avatar) => Some(avatar),
                Err(e) => {
                    tracing::warn!("Failed to load active avatar: {}", e);
                    None
                }
            }
        });

        Self {
            idle,
            thinking,
            active,
        }
    }

    /// Get the appropriate avatar for the given state
    pub fn get_for_state(&mut self, state: &crate::agent::AgentVisualState) -> Option<&mut Avatar> {
        use crate::agent::AgentVisualState;

        match state {
            AgentVisualState::Idle | AgentVisualState::Paused => self.idle.as_mut(),
            AgentVisualState::Thinking | AgentVisualState::Reading | AgentVisualState::Confused => {
                self.thinking.as_mut().or(self.idle.as_mut())
            }
            AgentVisualState::Writing | AgentVisualState::Happy => {
                self.active.as_mut().or(self.idle.as_mut())
            }
        }
    }

    /// Check if any avatars are loaded
    pub fn has_avatars(&self) -> bool {
        self.idle.is_some() || self.thinking.is_some() || self.active.is_some()
    }
}
