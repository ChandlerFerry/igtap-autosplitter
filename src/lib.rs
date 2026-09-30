#![cfg_attr(not(test), no_std)]
#![allow(non_camel_case_types, non_snake_case)]

use asr::{
    future::next_tick,
    game_engine::unity::{
        mono::{Class, Image, Module},
        scene_manager::SceneManager,
    },
    settings::Gui,
    Address, Address64, Process,
};

#[cfg(not(test))]
asr::async_main!(stable);
#[cfg(not(test))]
asr::panic_handler!();

#[derive(Gui, Clone, Copy, PartialEq, Debug)]
enum Mode {
    /// Full game: TouchGrass%, FalseEnding%, VmanCrash%, TrueEnding% (new game, first movement)
    #[default]
    FullGame,
    /// Course IL: start gate to end gate (Levels 1-15)
    CourseIl,
    /// VmanCrash% IL: Start a VMAN speedrun box to the Vman ending
    VmanIl,
}

#[derive(Gui)]
struct Settings {
    /// Run
    mode: Mode,
    /// Split: wall jump
    #[default = true]
    wall_jump: bool,
    /// Split: dash
    #[default = true]
    dash: bool,
    /// Split: double jump
    #[default = true]
    double_jump: bool,
    /// Split: block swap
    #[default = true]
    block_swap: bool,
    /// Split: omnidash
    #[default = true]
    omnidash: bool,
    /// Split: breaker trip
    #[default = true]
    breaker: bool,
    /// Split: landing in Area 2 (TouchGrass% end)
    #[default = true]
    area2: bool,
    /// Split: false ending ("You Win")
    #[default = true]
    false_ending: bool,
    /// Split: true ending (IGTAP logo)
    #[default = true]
    true_ending: bool,
    /// Split: Vman ending
    #[default = true]
    vman_ending: bool,
    /// Split: each course's first clear
    #[default = true]
    course_clears: bool,
}

impl Settings {
    /// The split toggles in `Snap::flags` order, then course clears.
    fn enabled(&self) -> [bool; FLAGS + 1] {
        [
            self.wall_jump,
            self.dash,
            self.double_jump,
            self.block_swap,
            self.omnidash,
            self.breaker,
            self.area2,
            self.false_ending,
            self.true_ending,
            self.vman_ending,
            self.course_clears,
        ]
    }
}

const FLAGS: usize = 10;
const COURSES: usize = 10;

#[derive(Clone, Copy, Default, Debug)]
struct Course {
    tracking: bool,
    time: f32,
    flashing: bool,
}

impl Course {
    /// `stopTracking` on a finish: `lastRunTextFlashing` goes on, or (in Vman) the time is kept.
    fn finished(prev: Course, now: Course) -> bool {
        prev.tracking && !now.tracking && (now.flashing || now.time != 0.0)
    }
}

#[derive(Clone, Copy, Default, Debug)]
struct Snap {
    /// The player's address: a new one is a new scene load.
    scene: u64,
    /// That load is a new game (the play timer near zero, not in Vman).
    fresh: bool,
    vman: bool,
    moved: bool,
    /// Wall jump, dash, double jump, block swap, omnidash, breaker, Area 2 landing, false, true, Vman ending.
    flags: [bool; FLAGS],
    courses: [Course; COURSES],
}

#[derive(Default)]
struct Run {
    armed: bool,
    course: usize,
    /// Events split this run (bit per flag, then per course clear).
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
    for bit in 0..FLAGS + COURSES {
        let hit = match bit.checked_sub(FLAGS) {
            None => same && enabled[bit] && !prev.flags[bit] && now.flags[bit],
            Some(course) => enabled[FLAGS] && finished(course),
        };
        if hit && run.done & 1 << bit == 0 {
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
    onGround: bool,
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
struct ZoneLoader {
    activeZone: i32,
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
struct EndCreditsTrigger {
    logo: Address64,
}

#[derive(Class)]
struct TMP_Text {
    m_fontColor: [f32; 4],
}

#[derive(Class)]
struct VmanScript {
    #[static_field]
    isCurrentlyVman: bool,
}

/// The scene's objects, found by their paths in `Overworld`.
struct Objects {
    scene: Address,
    player: Address,
    fresh: bool,
    stats: Address,
    zones: Address,
    fake: Option<Address>,
    end: Option<Address>,
    courses: [Option<Address>; COURSES],
}

struct Game {
    module: Module,
    scenes: SceneManager,
    movement: MovementBinding,
    stats: globalStatsBinding,
    zones: ZoneLoaderBinding,
    clock: timerBinding,
    course: courseScriptBinding,
    fake: FakeCreditsControlScriptBinding,
    end: EndCreditsTriggerBinding,
    text: TMP_TextBinding,
    vman: VmanScriptBinding,
}

impl Game {
    async fn attach(p: &Process) -> Game {
        let module = Module::wait_attach_auto_detect(p).await;
        let image = module.wait_get_default_image(p).await;
        let tmp: Image = module.wait_get_image(p, "Unity.TextMeshPro").await;
        let scenes = SceneManager::wait_attach(p).await;
        let movement = Movement::bind(p, &module, &image).await;
        let stats = globalStats::bind(p, &module, &image).await;
        let zones = ZoneLoader::bind(p, &module, &image).await;
        let clock = timer::bind(p, &module, &image).await;
        let course = courseScript::bind(p, &module, &image).await;
        let fake = FakeCreditsControlScript::bind(p, &module, &image).await;
        let end = EndCreditsTrigger::bind(p, &module, &image).await;
        let text = TMP_Text::bind(p, &module, &tmp).await;
        let vman = VmanScript::bind(p, &module, &image).await;
        Game {
            module,
            scenes,
            movement,
            stats,
            zones,
            clock,
            course,
            fake,
            end,
            text,
            vman,
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
            zones: self.component(p, &["Utils"], "ZoneLoader")?,
            fake: self.component(
                p,
                &["a1 overgrowth", "FakeCredits"],
                "FakeCreditsControlScript",
            ),
            end: self.component(p, &["Zone 3", "EndCreditsTrigger"], "EndCreditsTrigger"),
            courses,
        })
    }

    fn snap(&self, p: &Process, o: &Objects) -> Option<Snap> {
        let m = self.movement.read(p, o.player).ok()?;
        let vman = self.vman.read(p).ok()?.isCurrentlyVman;
        let credits = o
            .fake
            .is_some_and(|f| self.fake.read(p, f).is_ok_and(|f| f.creditsActive));
        let logo = o.end.and_then(|e| {
            let array = self.end.read(p, e).ok()?.logo;
            // A 64-bit Mono array's first element follows its 0x20-byte header.
            let first: Address64 = p.read(array + 0x20).ok()?;
            // The logo waits at alpha 1; the credits set it to 0 and fade it in within one frame.
            Some(self.text.read(p, first.into()).ok()?.m_fontColor[3] < 1.0)
        });
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
            flags: [
                m.wallJumpUnlocked,
                m.dashUnlocked,
                m.doubleJumpUnlocked,
                m.blockSwapUnlocked,
                m.omniDashUnlocked,
                self.stats.read(p, o.stats).ok()?.currentA1State == 1, // area1states.tripBreaker
                self.zones.read(p, o.zones).ok()?.activeZone == 2 && m.onGround,
                credits && !vman,
                logo == Some(true),
                credits && vman,
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
                    // A reload of the same scene can keep its address: look again twice a second.
                    if tick % 60 == 0 || objects.as_ref().is_none_or(|o| o.scene != scene) {
                        objects = game.find(&process, scene, objects.as_ref());
                    }
                    if let Some(now) = objects.as_ref().and_then(|o| game.snap(&process, o)) {
                        if let Some(prev) = prev {
                            let out =
                                step(settings.mode, &settings.enabled(), &mut run, &prev, &now);
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
        // Once per run: losing and regaining it (the Vman world) doesn't split again.
        assert_eq!(step(Mode::FullGame, &ALL, &mut run, &wj, &moved).splits, 0);
        assert_eq!(step(Mode::FullGame, &ALL, &mut run, &moved, &wj).splits, 0);
        // A new scene's loaded state isn't an event, and a Vman world isn't a new game.
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
        // A course's first clear splits, the second doesn't.
        let mut on = vman;
        on.courses[0] = Course {
            tracking: true,
            time: 1.0,
            flashing: false,
        };
        let off = Snap {
            courses: [Course {
                tracking: false,
                time: 0.0,
                flashing: true,
            }; COURSES],
            ..vman
        };
        assert_eq!(step(Mode::FullGame, &ALL, &mut run, &on, &off).splits, 1);
        assert_eq!(step(Mode::FullGame, &ALL, &mut run, &on, &off).splits, 0);
    }

    #[test]
    fn course_il() {
        let mut run = Run::default();
        let idle = Snap::default();
        let mut go = idle;
        go.courses[2] = Course {
            tracking: true,
            time: 0.02,
            flashing: false,
        };
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
        // A quick restart between reads: the time drops while tracking.
        assert!(step(Mode::CourseIl, &ALL, &mut run, &later, &go).start);
        // Dying stops tracking without a finish; the end gate stops it with one.
        let mut died = idle;
        died.courses[2].flashing = false;
        assert_eq!(
            step(Mode::CourseIl, &ALL, &mut run, &later, &died).splits,
            0
        );
        died.courses[2].flashing = true;
        assert_eq!(
            step(Mode::CourseIl, &ALL, &mut run, &later, &died).splits,
            1
        );
        // A Vman finish keeps its time instead.
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
        let mut end = bought;
        end.flags[9] = true;
        assert_eq!(step(Mode::VmanIl, &ALL, &mut run, &bought, &end).splits, 1);
    }
}
