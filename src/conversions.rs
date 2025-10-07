use fsct::{FsctStatus, PlayerState, TrackMetadata, TimelineInfo};
use roon_api::transport::{Zone, State};
use std::time::{Duration, SystemTime};

/// Convert Roon playback State to FSCT status
pub fn convert_status(roon_state: &State) -> FsctStatus {
    match roon_state {
        State::Playing => FsctStatus::Playing,
        State::Paused => FsctStatus::Paused,
        State::Stopped => FsctStatus::Stopped,
        State::Loading => FsctStatus::Buffering,
    }
}

/// Convert Roon zone to FSCT PlayerState
pub fn convert_zone_to_player_state(zone: &Zone) -> PlayerState {
    let status = convert_status(&zone.state);

    // Extract timeline if available
    let timeline = zone.now_playing.as_ref().and_then(|np| {
        np.seek_position.map(|seek_pos| {
            // seek_pos is in seconds (float), convert to Duration with millisecond precision
            let position = Duration::from_secs_f64(seek_pos as f64);
            let duration = np.length
                .map(|len| Duration::from_secs(len as u64))
                .unwrap_or(Duration::from_secs(0));

            TimelineInfo {
                position,
                update_time: SystemTime::now(),
                duration,
                rate: if zone.state == State::Playing { 1.0 } else { 0.0 },
            }
        })
    });

    // Extract metadata
    let texts = if let Some(now_playing) = &zone.now_playing {
        let three_line = &now_playing.three_line;
        TrackMetadata {
            title: Some(three_line.line1.clone()),
            artist: Some(three_line.line2.clone()),
            album: Some(three_line.line3.clone()),
            genre: None, // Roon doesn't provide genre in three_line
        }
    } else {
        TrackMetadata::default()
    };

    PlayerState {
        status,
        timeline,
        texts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_status() {
        assert_eq!(convert_status(&State::Playing), FsctStatus::Playing);
        assert_eq!(convert_status(&State::Paused), FsctStatus::Paused);
        assert_eq!(convert_status(&State::Stopped), FsctStatus::Stopped);
        assert_eq!(convert_status(&State::Loading), FsctStatus::Stopped);
    }
}
