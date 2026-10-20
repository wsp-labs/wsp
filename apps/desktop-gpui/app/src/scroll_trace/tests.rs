// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

fn frame(at: f64, draw: f64, present: f64) -> Frame {
    Frame { at, draw, draw_cpu: draw, switches: None, present, since: 0., late: 0., inputs: 0, focused: true, work: Work::default() }
}

fn input(due: f64, at: f64, phase: TouchPhase) -> Input {
    Input { due, at, phase, moving: true, dy: 10., spot: None, after: None }
}

#[test]
fn a_wheel_line_reads_back_as_written_and_as_the_event_a_platform_sends() {
    let line = r#"{"t":8.33,"dx":0,"dy":-12.5,"unit":"px","phase":"moved","x":420,"y":320.5}"#;
    let wheels = Wheel::parse_all(&format!("{line}\n\n{}\n", line.replace("8.33", "16.67").replace("moved", "ended"))).unwrap();
    assert_eq!(wheels.len(), 2);
    assert_eq!(wheels[0], Wheel { t: 8.33, dx: 0., dy: -12.5, lines: false, phase: TouchPhase::Moved, x: 420., y: 320.5 });
    assert_eq!(Wheel::parse_all(&wheels[0].line()).unwrap()[0], wheels[0]);
    let origin = point(px(300.), px(40.));
    let event = wheels[0].event(origin);
    assert_eq!(event.position, point(px(720.), px(360.5)));
    assert!(matches!(event.delta, ScrollDelta::Pixels(d) if d == point(px(0.), px(-12.5))));
    assert_eq!(Wheel::of(&event, 8.33, origin), wheels[0]);
    let lines = Wheel { lines: true, ..wheels[0].clone() };
    assert!(matches!(lines.event(origin).delta, ScrollDelta::Lines(d) if d == point(0., -12.5)));
    assert!(Wheel::parse_all(&line.replace("moved", "held")).unwrap_err().starts_with("line 1"));
    assert!(Wheel::parse_all(&format!("{}\n{line}", line.replace("8.33", "20"))).unwrap_err().contains("goes back"));
}

#[test]
fn an_input_waits_from_when_it_was_due_to_the_present_of_the_first_frame_after_it() {
    let mut frames = vec![frame(0., 5., 5.), frame(20., 5., 5.), frame(100., 5., 5.)];
    // Handled at 12 and 18, before the frame at 20: that frame shows both, presented at 30.
    let inputs = [input(10., 12., TouchPhase::Started), input(18., 18., TouchPhase::Moved), input(30., 95., TouchPhase::Moved)];
    settle(&mut frames, &inputs);
    assert_eq!((frames[0].inputs, frames[1].inputs, frames[2].inputs), (0, 2, 1));
    assert_eq!(frames[1].late, 20.);
    // Due at 30 by the replay's clock, handled at 95 behind a busy UI thread, presented at 110.
    assert_eq!(frames[2].late, 80.);
    assert!(!frames[1].slow() && frames[2].slow());
}

#[test]
fn a_moved_event_that_came_late_in_its_stream_waited_from_one_interval_after_the_one_before() {
    // A recording knows only when an event was handled; a Moved one 60 ms after the last waited behind the UI thread.
    let mut frames = vec![frame(0., 2., 2.), frame(70., 2., 2.)];
    let inputs = [input(0., 0., TouchPhase::Moved), input(60., 60., TouchPhase::Moved)];
    settle(&mut frames, &inputs);
    assert!((frames[1].late - (74. - STREAM_MS)).abs() < 1e-9, "{}", frames[1].late);
    // A Started event opens a new gesture, so the pause before it was the person's.
    let mut frames = vec![frame(0., 2., 2.), frame(70., 2., 2.)];
    settle(&mut frames, &[input(0., 0., TouchPhase::Moved), input(60., 60., TouchPhase::Started)]);
    assert_eq!(frames[1].late, 14.);
    // So was the pause after fingers came to rest, or between a mouse wheel's clicks.
    let mut frames = vec![frame(0., 2., 2.), frame(70., 2., 2.)];
    settle(&mut frames, &[Input { moving: false, ..input(0., 0., TouchPhase::Moved) }, input(60., 60., TouchPhase::Moved)]);
    assert_eq!(frames[1].late, 14.);
}

#[test]
fn a_frame_is_slow_by_its_draw_and_present_or_its_wait_and_its_app_time_leaves_the_present_out() {
    let mut f = frame(0., 30., 25.);
    assert!(f.slow(), "55 ms of draw and present");
    assert_eq!(f.app(), 30.);
    f.work.pages.push((200, 25.));
    assert!(f.app() > SLOW_MS, "a page before the draw is the UI thread's too");
    let plan = Plan { thread: Some("t".into()), replay: true, ..Plan::default() };
    let quick = frame(40., 4., 4.);
    let json = json(&plan, &[f, quick], &[], true, 2.);
    assert_eq!(json["summary"]["slow"], 1);
    assert_eq!(json["summary"]["app_slow"], 1);
    assert_eq!(json["slow"][0]["frame"], 0);
    assert_eq!(json["slow"][0]["pages"][0]["events"], 200);
    assert_eq!((json["mode"].as_str(), json["frames"].as_array().map(Vec::len)), (Some("replay"), Some(2)));
}

#[test]
fn wheel_events_that_find_room_and_move_nothing_make_a_stuck_run_named_by_what_was_under_the_pointer() {
    let over = |top: f32, after: Option<f32>, dy: f32, under: &str| Input {
        dy,
        spot: Some(Spot { top, max: 1000., under: Some(under.to_owned()) }),
        after,
        ..input(0., 0., TouchPhase::Moved)
    };
    let mut inputs: Vec<Input> = vec![over(500., Some(490.), 10., "Agent")];
    inputs.extend((0..5).map(|_| over(490., Some(490.), 10., "ChangedFiles")));
    inputs.push(over(490., Some(480.), 10., "Agent"));
    // At the top an upward wheel has no room, and two still events are a frame's worth, not a stuck run.
    inputs.extend((0..4).map(|_| over(0., Some(0.), 10., "Person")));
    inputs.extend([over(480., Some(480.), -10., "Calls"), over(480., Some(480.), -10., "Calls"), over(480., Some(470.), -10., "Calls")]);
    // A recording reads where the list stood from the next event when it could not read it after the event.
    inputs.extend((0..3).map(|_| over(470., None, 10., "Agent with code")));
    inputs.push(over(470., None, 10., "Agent"));
    let runs = stuck(&inputs);
    assert_eq!(runs.len(), 2, "{runs:?}");
    assert_eq!((runs[0].events, runs[0].under.as_deref()), (5, Some("ChangedFiles")));
    assert_eq!((runs[1].events, runs[1].under.as_deref()), (3, Some("Agent with code")));
}
