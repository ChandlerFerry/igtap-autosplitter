#![cfg_attr(not(test), no_std)]
#![allow(non_camel_case_types, non_snake_case)]

use asr::{
    future::next_tick,
    game_engine::unity::{
        mono::{Class, Image, Module},
        scene_manager::SceneManager,
    },
    settings::Gui,
    Address, Process,
};

#[cfg(not(test))]
asr::async_main!(stable);
#[cfg(not(test))]
asr::panic_handler!();

#[derive(Gui, Clone, Copy, PartialEq, Debug)]
enum Mode {
    #[doc = "Full game"]
    #[default]
    FullGame,
    #[doc = "Course IL"]
    CourseIl,
    #[doc = "VmanCrash% IL"]
    VmanIl,
}

#[derive(Gui)]
struct Settings {
    run: Mode,
    #[default = true]
    wall_jump: bool,
    #[default = true]
    dash: bool,
    #[default = true]
    double_jump: bool,
    #[default = true]
    block_swap: bool,
    #[default = true]
    omnidash: bool,
    #[default = true]
    breaker: bool,
    #[default = true]
    touch_grass: bool,
    #[default = true]
    false_ending: bool,
    #[default = true]
    true_ending: bool,
    #[default = true]
    vman_ending: bool,
    #[default = true]
    course_clears: bool,
}

impl Settings {
    fn enabled(&self) -> [bool; FLAGS + 1] {
        [
            self.wall_jump,
            self.dash,
            self.double_jump,
            self.block_swap,
            self.omnidash,
            self.breaker,
            self.touch_grass,
            self.false_ending,
            self.true_ending,
            self.vman_ending,
            self.course_clears,
        ]
    }
}

const FLAGS: usize = 10;
const COURSES: usize = 10;
const OVERGROWN: usize = 5;
const DASH: usize = 1;
const OMNIDASH: usize = 4;

#[derive(Clone, Copy, Default, Debug)]
struct Course {
    tracking: bool,
    time: f32,
    flashing: bool,
}

impl Course {
    fn finished(prev: Course, now: Course) -> bool {
        prev.tracking && !now.tracking && (now.flashing || now.time != 0.0)
    }
}

#[derive(Clone, Copy, Default, Debug)]
struct Snap {
    scene: u64,
    fresh: bool,
    vman: bool,
    moved: bool,
    overgrown: bool,
    dashes: i32,
    flags: [bool; FLAGS],
    courses: [Course; COURSES],
}

#[derive(Default)]
struct Run {
    armed: bool,
    course: usize,
    done: u32,
}

#[derive(Default, Debug, PartialEq)]
struct Out {
    reset: bool,
    start: bool,
    splits: u32,
}

fn step(mode: Mode, enabled: &[bool; FLAGS + 1], run: &mut Run, prev: &Snap, now: &Snap) -> Out {
    let mut out = Out::default();
    let same = prev.scene == now.scene;
    let finished = |i: usize| same && Course::finished(prev.courses[i], now.courses[i]);
    match mode {
        Mode::FullGame => {
            if !same && now.fresh && !now.vman {
                run.armed = true;
                out.reset = true;
            }
            if run.armed && now.moved {
                run.armed = false;
                out.start = true;
            }
        }
        Mode::CourseIl => {
            for i in 0..COURSES {
                let (p, c) = (prev.courses[i], now.courses[i]);
                if same && c.tracking && (!p.tracking || c.time < p.time) {
                    run.course = i;
                    out.reset = true;
                    out.start = true;
                } else if finished(i) && i == run.course {
                    out.splits += 1;
                }
            }
            return out;
        }
        Mode::VmanIl => {
            if !prev.vman && now.vman {
                out.reset = true;
                out.start = true;
            }
        }
    }
    if out.start {
        run.done = 0;
    }
    let rose = |i: usize| match i {
        DASH => now.dashes > prev.dashes && now.flags[OMNIDASH] == prev.flags[OMNIDASH],
        _ => !prev.flags[i] && now.flags[i],
    };
    let flags = (0..FLAGS).filter(|&i| same && enabled[i] && rose(i));
    let clears = (0..COURSES)
        .filter(|&i| enabled[FLAGS] && finished(i))
        .map(|i| {
            FLAGS
                + i
                + if now.overgrown && i < OVERGROWN {
                    COURSES
                } else {
                    0
                }
        });
    for bit in flags.chain(clears) {
        if run.done & 1 << bit == 0 {
            run.done |= 1 << bit;
            out.splits += 1;
        }
    }
    out
}

#[derive(Class)]
struct Movement {
    MoveAxis: [f32; 2],
    jumpBuffer: bool,
    cutsceneMode: i32,
    longfallMult: f32,
    maxAirDashes: i32,
    wallJumpUnlocked: bool,
    dashUnlocked: bool,
    doubleJumpUnlocked: bool,
    blockSwapUnlocked: bool,
    omniDashUnlocked: bool,
}

#[derive(Class)]
struct globalStats {
    currentA1State: i32,
}

#[derive(Class)]
struct timer {
    time: f64,
}

#[derive(Class)]
struct courseScript {
    courseNumber: i32,
    tracking: bool,
    currentPathTime: f32,
    lastRunTextFlashing: bool,
}

#[derive(Class)]
struct FakeCreditsControlScript {
    creditsActive: bool,
}

#[derive(Class)]
struct TMP_Text {
    m_isAwake: bool,
}

#[derive(Class)]
struct VmanScript {
    #[static_field]
    isCurrentlyVman: bool,
}

struct Objects {
    scene: Address,
    player: Address,
    fresh: bool,
    stats: Address,
    fake: Option<Address>,
    credits: Option<Address>,
    courses: [Option<Address>; COURSES],
}

struct Game {
    module: Module,
    scenes: SceneManager,
    movement: MovementBinding,
    stats: globalStatsBinding,
    clock: timerBinding,
    course: courseScriptBinding,
    fake: FakeCreditsControlScriptBinding,
    text: TMP_TextBinding,
    vman: VmanScriptBinding,
}

impl Game {
    async fn attach(p: &Process) -> Game {
        let module = Module::wait_attach_auto_detect(p).await;
        let image = module.wait_get_default_image(p).await;
        let tmp: Image = module.wait_get_image(p, "Unity.TextMeshPro").await;
        Game {
            scenes: SceneManager::wait_attach(p).await,
            movement: Movement::bind(p, &module, &image).await,
            stats: globalStats::bind(p, &module, &image).await,
            clock: timer::bind(p, &module, &image).await,
            course: courseScript::bind(p, &module, &image).await,
            fake: FakeCreditsControlScript::bind(p, &module, &image).await,
            text: TMP_Text::bind(p, &module, &tmp).await,
            vman: VmanScript::bind(p, &module, &image).await,
            module,
        }
    }

    fn component(&self, p: &Process, path: &[&str], class: &str) -> Option<Address> {
        let mut t = self.scenes.get_root_game_object(p, path[0]).ok()?;
        for name in &path[1..] {
            t = t.get_child(p, &self.scenes, name).ok()?;
        }
        t.get_component_mono(p, &self.scenes, &self.module, class)
            .ok()
    }

    fn find(&self, p: &Process, scene: Address, old: Option<&Objects>) -> Option<Objects> {
        let player = self.component(p, &["PlayerObject", "Player"], "Movement")?;
        let clock = self.component(p, &["Utils"], "timer")?;
        let fresh = match old {
            Some(o) if o.player == player => o.fresh,
            _ => self.clock.read(p, clock).ok()?.time < 5.0,
        };
        let mut courses = [None; COURSES];
        if let Ok(root) = self.scenes.get_root_game_object(p, "Courses") {
            for t in root.children(p, &self.scenes).ok()? {
                let Ok(c) = t.get_component_mono(p, &self.scenes, &self.module, "courseScript")
                else {
                    continue;
                };
                let Ok(n) = self.course.read(p, c).map(|c| c.courseNumber as usize) else {
                    continue;
                };
                if let Some(slot) = courses.get_mut(n.wrapping_sub(1)) {
                    *slot = Some(c);
                }
            }
        }
        Some(Objects {
            scene,
            player,
            fresh,
            stats: self.component(p, &["Utils"], "globalStats")?,
            fake: self.component(
                p,
                &["a1 overgrowth", "FakeCredits"],
                "FakeCreditsControlScript",
            ),
            credits: self.component(
                p,
                &[
                    "Zone 3",
                    "Background",
                    "background3_7 2",
                    "CreditsObject",
                    "credits",
                    "PTgames",
                ],
                "TextMeshProUGUI",
            ),
            courses,
        })
    }

    fn snap(&self, p: &Process, o: &Objects) -> Option<Snap> {
        let m = self.movement.read(p, o.player).ok()?;
        let vman = self.vman.read(p).ok()?.isCurrentlyVman;
        let a1 = self.stats.read(p, o.stats).ok()?.currentA1State;
        let fake = o
            .fake
            .is_some_and(|f| self.fake.read(p, f).is_ok_and(|f| f.creditsActive));
        let credits = o
            .credits
            .is_some_and(|c| self.text.read(p, c).is_ok_and(|t| t.m_isAwake));
        let mut courses = [Course::default(); COURSES];
        for (slot, c) in courses.iter_mut().zip(o.courses) {
            if let Some(c) = c.and_then(|c| self.course.read(p, c).ok()) {
                *slot = Course {
                    tracking: c.tracking,
                    time: c.currentPathTime,
                    flashing: c.lastRunTextFlashing,
                };
            }
        }
        Some(Snap {
            scene: o.player.value(),
            fresh: o.fresh,
            vman,
            moved: m.MoveAxis != [0.0; 2] || m.jumpBuffer,
            overgrown: a1 == 2,
            dashes: m.maxAirDashes,
            flags: [
                m.wallJumpUnlocked,
                m.dashUnlocked,
                m.doubleJumpUnlocked,
                m.blockSwapUnlocked,
                m.omniDashUnlocked,
                a1 == 1,
                m.cutsceneMode == 3 && m.longfallMult == 1.1,
                fake && !vman,
                credits,
                fake && vman,
            ],
            courses,
        })
    }
}

#[cfg_attr(test, allow(dead_code))]
async fn main() {
    let mut settings = Settings::register();
    loop {
        let process = Process::wait_attach("IGTAPfullGame.exe").await;
        process
            .until_closes(async {
                let game = Game::attach(&process).await;
                let mut objects: Option<Objects> = None;
                let mut prev: Option<Snap> = None;
                let mut run = Run::default();
                for tick in 0u32.. {
                    settings.update();
                    let scene = game
                        .scenes
                        .get_current_scene(&process)
                        .map_or(Address::NULL, |s| s.address());
                    if tick % 60 == 0 || objects.as_ref().is_none_or(|o| o.scene != scene) {
                        objects = game.find(&process, scene, objects.as_ref());
                    }
                    if let Some(now) = objects.as_ref().and_then(|o| game.snap(&process, o)) {
                        if let Some(prev) = prev {
                            let out =
                                step(settings.run, &settings.enabled(), &mut run, &prev, &now);
                            if out.reset {
                                asr::timer::reset();
                            }
                            if out.start {
                                asr::timer::start();
                            }
                            for _ in 0..out.splits {
                                asr::timer::split();
                            }
                        }
                        prev = Some(now);
                    }
                    next_tick().await;
                }
            })
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [bool; FLAGS + 1] = [true; FLAGS + 1];

    fn course(tracking: bool, time: f32, flashing: bool) -> Course {
        Course {
            tracking,
            time,
            flashing,
        }
    }

    #[test]
    fn full_game() {
        let mut run = Run::default();
        let old = Snap {
            scene: 1,
            ..Snap::default()
        };
        let load = Snap {
            scene: 2,
            fresh: true,
            ..Snap::default()
        };
        assert_eq!(
            step(Mode::FullGame, &ALL, &mut run, &old, &load),
            Out {
                reset: true,
                ..Out::default()
            }
        );
        let moved = Snap {
            moved: true,
            ..load
        };
        assert_eq!(
            step(Mode::FullGame, &ALL, &mut run, &load, &moved),
            Out {
                start: true,
                ..Out::default()
            }
        );
        let mut wj = moved;
        wj.flags[0] = true;
        assert_eq!(step(Mode::FullGame, &ALL, &mut run, &moved, &wj).splits, 1);
        assert_eq!(step(Mode::FullGame, &ALL, &mut run, &wj, &moved).splits, 0);
        assert_eq!(step(Mode::FullGame, &ALL, &mut run, &moved, &wj).splits, 0);
        let mut vman = Snap {
            scene: 3,
            fresh: true,
            vman: true,
            ..Snap::default()
        };
        vman.flags[1] = true;
        assert_eq!(
            step(Mode::FullGame, &ALL, &mut run, &wj, &vman),
            Out::default()
        );
        let mut on = vman;
        on.courses[0] = course(true, 1.0, false);
        let mut off = vman;
        off.courses[0] = course(false, 0.0, true);
        assert_eq!(step(Mode::FullGame, &ALL, &mut run, &on, &off).splits, 1);
        assert_eq!(step(Mode::FullGame, &ALL, &mut run, &on, &off).splits, 0);
        let (on, off) = (
            Snap {
                overgrown: true,
                ..on
            },
            Snap {
                overgrown: true,
                ..off
            },
        );
        assert_eq!(step(Mode::FullGame, &ALL, &mut run, &on, &off).splits, 1);
        assert_eq!(step(Mode::FullGame, &ALL, &mut run, &on, &off).splits, 0);
    }

    #[test]
    fn false_ending_route() {
        fn push(t: &mut Vec<(Snap, u32)>, change: impl Fn(&mut Snap), splits: u32) {
            let mut s = t.last().unwrap().0;
            change(&mut s);
            t.push((s, splits));
        }
        fn lap(t: &mut Vec<(Snap, u32)>, i: usize, finish: bool) {
            let first = !t.iter().any(|(s, _)| s.courses[i].flashing);
            push(t, |s| s.courses[i] = course(true, 0.0, false), 0);
            push(t, |s| s.courses[i].time = 3.0, 0);
            push(
                t,
                |s| s.courses[i] = course(false, 0.0, finish),
                (finish && first) as u32,
            );
        }
        let new_game = Snap {
            scene: 1,
            fresh: true,
            moved: true,
            ..Snap::default()
        };
        let t = &mut vec![(Snap::default(), 0), (new_game, 0)];
        lap(t, 0, false);
        lap(t, 0, true);
        push(t, |s| s.flags[0] = true, 1);
        lap(t, 1, true);
        lap(t, 0, true);
        push(t, |s| (s.flags[DASH], s.dashes) = (true, 1), 1);
        lap(t, 2, false);
        lap(t, 2, true);
        lap(t, 1, true);
        push(t, |s| s.flags[2] = true, 1);
        push(t, |s| s.flags[7] = true, 1);
        let mut run = Run::default();
        for w in t.windows(2) {
            assert_eq!(
                step(Mode::FullGame, &ALL, &mut run, &w[0].0, &w[1].0).splits,
                w[1].1
            );
        }
        assert_eq!(t.iter().map(|(_, n)| n).sum::<u32>(), 7);
    }

    #[test]
    fn course_il() {
        let mut run = Run::default();
        let idle = Snap::default();
        let mut go = idle;
        go.courses[2] = course(true, 0.02, false);
        assert_eq!(
            step(Mode::CourseIl, &ALL, &mut run, &idle, &go),
            Out {
                reset: true,
                start: true,
                splits: 0
            }
        );
        let mut later = go;
        later.courses[2].time = 3.0;
        assert!(step(Mode::CourseIl, &ALL, &mut run, &later, &go).start);
        let mut left = idle;
        left.courses[2].flashing = false;
        assert_eq!(
            step(Mode::CourseIl, &ALL, &mut run, &later, &left).splits,
            0
        );
        left.courses[2].flashing = true;
        assert_eq!(
            step(Mode::CourseIl, &ALL, &mut run, &later, &left).splits,
            1
        );
        let mut vman = idle;
        vman.courses[2].time = 3.02;
        assert_eq!(
            step(Mode::CourseIl, &ALL, &mut run, &later, &vman).splits,
            1
        );
    }

    #[test]
    fn vman_il() {
        let mut run = Run::default();
        let before = Snap::default();
        let bought = Snap {
            vman: true,
            ..before
        };
        assert_eq!(
            step(Mode::VmanIl, &ALL, &mut run, &before, &bought),
            Out {
                reset: true,
                start: true,
                splits: 0
            }
        );
        let mut omni = bought;
        omni.flags[DASH] = true;
        omni.flags[OMNIDASH] = true;
        omni.dashes = 1;
        assert_eq!(step(Mode::VmanIl, &ALL, &mut run, &bought, &omni).splits, 1);
        let dash = Snap { dashes: 2, ..omni };
        assert_eq!(step(Mode::VmanIl, &ALL, &mut run, &omni, &dash).splits, 1);
        let mut end = dash;
        end.flags[9] = true;
        assert_eq!(step(Mode::VmanIl, &ALL, &mut run, &dash, &end).splits, 1);
    }
}
