use fsct::{FsctStatus, PlayerState, TrackMetadata, TimelineInfo};
use roon_api::transport::{Zone, ZoneSeek, State};
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

/// Convert Roon zone to FSCT TimelineInfo (for seek-only updates)
pub fn convert_zone_to_timeline_info(zone: &Zone) -> Option<TimelineInfo> {
    zone.now_playing.as_ref().and_then(|np| {
        // when position is not available, just ignore it and 0
        let position = np.seek_position.map(|seek_pos| Duration::from_secs_f64(seek_pos as f64)).unwrap_or_default();

        // when length is not available, whole TimelineInfo is not available
        np.length.map(|len| {
            let duration = Duration::from_secs_f64(len as f64);

            TimelineInfo {
                position,
                update_time: SystemTime::now(),
                duration,
                rate: if zone.state == State::Playing { 1.0 } else { 0.0 },
            }
        })
    })
}

/// Build TimelineInfo from ZoneSeek, using cached timeline for rate preservation
/// This is used for seek-only updates where we need to preserve the playback rate
pub fn build_timeline_from_seek(
    cached: Option<&TimelineInfo>,
    zone_seek: &ZoneSeek,
) -> Option<TimelineInfo> {
    zone_seek.seek_position.map(|seek_pos| {
        let position = Duration::from_secs(seek_pos as u64);
        let duration = Duration::from_secs((zone_seek.queue_time_remaining + seek_pos) as u64);

        // Use cached rate if available, otherwise default to 1.0
        let rate = cached.map(|t| t.rate).unwrap_or(1.0);

        TimelineInfo {
            position,
            update_time: SystemTime::now(),
            duration,
            rate,
        }
    })
}

/// Convert Roon zone to FSCT PlayerState
pub fn convert_zone_to_player_state(zone: &Zone) -> PlayerState {
    let status = convert_status(&zone.state);

    // Extract timeline if available
    let timeline = convert_zone_to_timeline_info(zone);

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
        assert_eq!(convert_status(&State::Loading), FsctStatus::Buffering);
    }
}
