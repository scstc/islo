//! islo的原生矢量表情与连续形变。
//! Mouth vocabulary and expression mapping adapted from Aora Bot, Copyright (c) 2026 sam70361.
//! Aora Bot License 1.1: third_party/aora-bot/LICENSE; source and revision in NOTICE.
use crate::anim::{approach, lerp, smoothstep};
use crate::paint::{Card, H};
use crate::theme::{self, W};
use eframe::egui::Color32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PetState {
    Awake,
    Curious,
    Waking,
    Happy,
    Satisfied,
    Music,
    Worried,
    Focused,
    Dozing,
    Sleeping,
    Listening,
    Thinking,
    Loading,
    Excited,
    Laughing,
    Shy,
    Sad,
    Angry,
    Surprised,
    Confused,
}

impl PetState {
    pub const ALL:[Self;20]=[Self::Awake,Self::Curious,Self::Waking,Self::Happy,Self::Satisfied,Self::Music,Self::Worried,Self::Focused,Self::Dozing,Self::Sleeping,Self::Listening,Self::Thinking,Self::Loading,Self::Excited,Self::Laughing,Self::Shy,Self::Sad,Self::Angry,Self::Surprised,Self::Confused];
    pub fn label(self,locale:crate::i18n::Locale)->&'static str {
        locale.text(match self {Self::Awake=>"待机",Self::Curious=>"好奇",Self::Waking=>"苏醒",Self::Happy=>"开心",Self::Satisfied=>"满足",Self::Music=>"听音乐",Self::Worried=>"担心",Self::Focused=>"专注",Self::Dozing=>"困倦",Self::Sleeping=>"睡眠",Self::Listening=>"倾听",Self::Thinking=>"思考",Self::Loading=>"加载",Self::Excited=>"兴奋",Self::Laughing=>"大笑",Self::Shy=>"害羞",Self::Sad=>"难过",Self::Angry=>"生气",Self::Surprised=>"惊讶",Self::Confused=>"疑惑"})
    }
}

#[derive(Clone,Copy,Debug)]
struct Mouth {width:f32,bend:f32,open:f32,wave:f32,skew:f32}
impl Mouth {
    // Aora Bot's line-face mouth units, fitted to islo's compact resident slot.
    fn target(state:PetState)->Self {
        let (width,bend,open,wave,skew)=match state {
            PetState::Happy|PetState::Satisfied=>(0.6,0.22,0.0,0.0,0.0),
            PetState::Excited=>(0.7,0.26,0.14,0.0,0.0),
            PetState::Laughing=>(0.5,0.14,0.62,0.0,0.0),
            PetState::Shy=>(0.5,0.15,0.0,0.0,0.7),
            PetState::Surprised=>(0.24,0.0,0.48,0.0,0.0),
            PetState::Focused|PetState::Curious=>(0.1,0.0,0.2,0.0,0.0),
            PetState::Confused|PetState::Worried=>(0.55,0.0,0.0,0.11,0.0),
            PetState::Sad|PetState::Angry=>(0.5,-0.18,0.0,0.0,0.0),
            PetState::Dozing=>(0.3,-0.1,0.0,0.0,0.0),
            PetState::Thinking|PetState::Loading=>(0.32,0.0,0.0,0.0,0.0),
            PetState::Listening|PetState::Sleeping=>(0.5,0.0,0.0,0.0,0.0),
            _=>(0.5,0.09,0.0,0.0,0.0),
        };
        Self {width:width*5.25,bend:bend*5.25,open:open*5.25,wave:wave*5.25,skew}
    }
    fn follow(&mut self,goal:Self,dt:f32) {
        self.width=approach(self.width,goal.width,14.0,dt);self.bend=approach(self.bend,goal.bend,14.0,dt);
        self.open=approach(self.open,goal.open,14.0,dt);self.wave=approach(self.wave,goal.wave,14.0,dt);self.skew=approach(self.skew,goal.skew,14.0,dt);
    }
    fn draw(self,c:&Card,cx:f32,y:f32) {
        let point=|u:f32,v:f32|(cx+u*self.width,y+self.bend*(1.0-u*u)+v*self.open/2.0+(u*std::f32::consts::TAU).sin()*self.wave+u*self.skew*0.5);
        let points:Vec<_>=if self.open>0.08 {(0..=32).map(|i|{let a=i as f32/32.0*std::f32::consts::TAU;point(a.cos(),a.sin())}).collect()}
            else {(0..=16).map(|i|point(i as f32/8.0-1.0,0.0)).collect()};
        c.path(&points,0.85,theme::WHITE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore="按需导出 islo 全部表情的连续帧"]
    fn expression_preview_gallery() {
        use eframe::egui::{self,Pos2,Rect,Vec2};
        let dest=std::path::PathBuf::from(std::env::var_os("ISLO_PET_PREVIEW_DIR").expect("需指定预览目录"));
        std::fs::create_dir_all(&dest).unwrap();
        let old=std::env::var_os("ISLO_DATA_DIR");std::env::set_var("ISLO_DATA_DIR",dest.join("data"));
        let ctx=egui::Context::default();let _app=crate::app::App::new(&ctx,crate::app::Options{fetch_lyrics:false,pet_idle:1e9});
        let mut pets:Vec<_>=PetState::ALL.into_iter().map(|_|Pet::default()).collect();
        let mut raster=crate::preview_raster::Raster::new(800,480);
        for frame in 0..48 {
            let t=frame as f64/24.0;
            let out=ctx.run(egui::RawInput{screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(800.0,480.0))),time:Some(t),..Default::default()},|ctx|{
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx,|ui|{
                    ui.painter().rect_filled(ui.max_rect(),0.0,Color32::from_rgb(12,14,18));
                    ui.painter().text(Pos2::new(18.0,20.0),egui::Align2::LEFT_CENTER,"islo · 20 expressions",egui::FontId::proportional(18.0),theme::WHITE);
                    for (index,(pet,state)) in pets.iter_mut().zip(PetState::ALL).enumerate() {
                        let origin=Pos2::new((index%5)as f32*160.0,(index/5)as f32*110.0+40.0);
                        let rect=Rect::from_min_size(origin,Vec2::new(160.0,110.0));
                        ui.painter().rect_filled(rect.shrink(5.0),10.0,Color32::from_rgb(23,26,32));
                        pet.step(state,t,1.0/24.0,[0.0;2],true);
                        let mut c=Card::new(ui,origin+Vec2::new(0.0,12.0),rect);c.s=2.0;pet.draw(&c,40.0,t);
                        ui.painter().text(origin+Vec2::new(80.0,88.0),egui::Align2::CENTER_CENTER,state.label(crate::i18n::Locale::En),egui::FontId::proportional(12.0),Color32::from_gray(190));
                    }
                });
            });
            raster.update_textures(&out.textures_delta);raster.buf.fill([0.0,0.0,0.0,1.0]);
            raster.draw(&ctx.tessellate(out.shapes,out.pixels_per_point),out.pixels_per_point);
            let mut rgba=Vec::new();raster.to_rgba8(&mut rgba);
            image::save_buffer(dest.join(format!("frame-{frame:03}.png")),&rgba,800,480,image::ColorType::Rgba8).unwrap();
            if frame==47 {image::save_buffer(dest.join("expressions.png"),&rgba,800,480,image::ColorType::Rgba8).unwrap();}
            raster.free_textures(&out.textures_delta);
        }
        if let Some(old)=old {std::env::set_var("ISLO_DATA_DIR",old);}else{std::env::remove_var("ISLO_DATA_DIR");}
    }
    #[test]
    fn expression_changes_morph_and_hidden_previews_keep_their_playback_time() {
        let mut pet=Pet::default();pet.react(PetState::Laughing);
        pet.advance(10.0,false);assert_eq!(pet.reaction,Some((PetState::Laughing,2.6)));
        pet.step(PetState::Laughing,0.0,0.016,[0.0;2],true);
        assert!(pet.mouth.open>0.0 && pet.mouth.open<Mouth::target(PetState::Laughing).open,"嘴型应逐步过渡，不能瞬间跳变");
        for state in PetState::ALL {
            for frame in 0..90 {pet.step(state,frame as f64/60.0,1.0/60.0,[0.0;2],true);}
            assert!((pet.mouth.open-Mouth::target(state).open).abs()<0.01);
            assert!(!state.label(crate::i18n::Locale::En).chars().any(|c|('\u{4e00}'..='\u{9fff}').contains(&c)));
        }
        pet.advance(2.7,true);assert!(pet.reaction.is_none(),"预览结束后交回自动状态");
    }
    #[test]
    fn singing_keeps_each_expression_for_fifteen_seconds_after_the_transition() {
        let mut pet=Pet::default();
        pet.step(PetState::Music,0.0,0.0,[0.0;2],true);
        let pose=pet.music_pose();
        for frame in 1..960 {
            pet.step(PetState::Music,frame as f64/60.0,1.0/60.0,[0.0;2],true);
            assert_eq!(pet.music_pose(),pose,"一秒过渡和十五秒停留期间不能频繁换表情");
        }
        pet.step(PetState::Music,16.1,0.2,[0.0;2],true);
        assert_ne!(pet.music_pose(),pose);
    }
    #[test]
    fn singing_rotates_expressions_and_starts_with_the_microphone() {
        let mut pet=Pet::default();let mut poses=Vec::new();let mut eyes=Vec::new();
        for frame in 0..=5880 {
            pet.step(PetState::Music,frame as f64/60.0,1.0/60.0,[0.0;2],true);
            if [60,1020,1980,2940,3900,4860,5820].contains(&frame) {
                poses.push(pet.music_pose());
                eyes.push(pet.eyes.map(|eye|(eye.height,eye.curve)));
            }
        }
        assert_eq!(poses,vec![MusicPose::Microphone,MusicPose::Hum,MusicPose::SideSing,MusicPose::Squint,MusicPose::HighNote,MusicPose::Smile,MusicPose::Microphone]);
        assert_ne!(eyes[0],eyes[2]);assert_ne!(eyes[2],eyes[3]);assert_ne!(eyes[3],eyes[4]);
        let age=pet.age;
        pet.step(PetState::Music,100.0,20.0,[0.0;2],false);
        assert_eq!(pet.age,age,"展开面板时不应跳过演唱表情");
        pet.step(PetState::Awake,101.0,0.1,[0.0;2],true);
        pet.step(PetState::Music,102.0,0.1,[0.0;2],true);
        assert_eq!(pet.music_pose(),MusicPose::Microphone);
    }
    #[test]
    fn microphone_moves_gently_while_the_head_keeps_facing_the_mouth() {
        let mut heights=Vec::new();let mut angles=Vec::new();
        for frame in 0..600 {
            let (head,angle)=microphone_pose(frame as f32/60.0);
            let tip=[head[0]+7.0*angle.cos(),head[1]+7.0*angle.sin()];
            let distance=|p:[f32;2]|((p[0]+0.8).powi(2)+(p[1]-20.7).powi(2)).sqrt();
            assert!(distance(head)<distance(tip),"麦头应在嘴边，麦柄朝外");
            assert!(head[0]-2.6>1.4 && head[1]-2.6>17.0,"不能遮住嘴或眼睛");
            assert!(tip[0]+1.0<16.0 && tip[1]+1.0<28.0,"摆动不能超出常驻槽位");
            heights.push(head[1]);angles.push(angle);
        }
        let range=|values:Vec<f32>|values.iter().copied().fold(f32::NEG_INFINITY,f32::max)-values.iter().copied().fold(f32::INFINITY,f32::min);
        assert!((0.6..1.1).contains(&range(heights)),"麦头应有细微起伏");
        assert!((0.10..0.18).contains(&range(angles)),"麦柄应有轻微转动");
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MusicPose { Microphone, Hum, SideSing, Squint, HighNote, Smile }

const MUSIC_PERIOD:f32=16.0;
fn music_pose_at(age:f32)->MusicPose {
    match (age/MUSIC_PERIOD) as usize%6 {0=>MusicPose::Microphone,1=>MusicPose::Hum,2=>MusicPose::SideSing,3=>MusicPose::Squint,4=>MusicPose::HighNote,_=>MusicPose::Smile}
}

fn microphone_pose(age:f32)->([f32;2],f32) {
    ([5.9+(age*0.95).sin()*0.2,20.8+(age*1.35).sin()*0.45],35.0_f32.to_radians()+(age*1.1).sin()*0.07)
}

fn microphone(c:&Card,cx:f32,y:f32,age:f32){
    let age=if crate::bar_layout::animations_enabled(){age}else{0.0};
    let (head,angle)=microphone_pose(age);
    let point=|s:f32,v:f32|(cx+head[0]+s*angle.cos()-v*angle.sin(),y+head[1]+s*angle.sin()+v*angle.cos());
    let capsule=|start:f32,end:f32,width:f32,color:Color32|{
        let a=point(start,0.0);let b=point(end,0.0);
        c.seg(a,b,width,color);c.circle(a.0,a.1,width/2.0,color);c.circle(b.0,b.1,width/2.0,color);
    };
    let line=|a:(f32,f32),b:(f32,f32),width:f32,color:Color32|c.seg(point(a.0,a.1),point(b.0,b.1),width,color);
    capsule(2.3,7.0,1.9,Color32::from_rgb(176,183,197));
    capsule(2.5,6.8,1.45,Color32::from_rgb(76,84,101));
    line((3.4,-0.53),(6.4,-0.53),0.28,Color32::from_rgb(218,225,237));
    capsule(2.05,2.65,2.6,Color32::from_rgb(126,137,157));
    let head=point(0.0,0.0);
    c.circle(head.0,head.1,2.6,Color32::from_rgb(222,228,238));
    c.circle(head.0,head.1,2.15,Color32::from_rgb(164,176,194));
    for s in [-1.0f32,0.0,1.0] {let v=(4.0-s*s).sqrt()*0.88;line((s,-v),(s,v),0.32,Color32::from_rgb(92,105,126));}
    for v in [-1.0f32,0.0,1.0] {let s=(4.0-v*v).sqrt()*0.88;line((-s,v),(s,v),0.3,Color32::from_rgb(226,232,243));}
}

#[derive(Clone, Copy)]
struct Eye {
    height: f32,
    spread: f32,
    curve: f32,
    y: f32,
    tilt: f32,
}

impl Eye {
    fn open(height: f32) -> Self { Self { height, spread: 0.0, curve: 0.0, y: 0.0, tilt: 0.0 } }
    fn arc(spread: f32, curve: f32) -> Self { Self { spread, curve, ..Self::open(0.0) } }
    fn follow(&mut self, goal: Self, dt: f32) {
        self.height = approach(self.height, goal.height, 17.0, dt);
        self.spread = approach(self.spread, goal.spread, 17.0, dt);
        self.curve = approach(self.curve, goal.curve, 17.0, dt);
        self.y = approach(self.y, goal.y, 12.0, dt);
        self.tilt = approach(self.tilt, goal.tilt, 12.0, dt);
    }
}

pub(crate) struct Pet {
    pub state: PetState,
    pub preview: PetState,
    pub reaction: Option<(PetState, f32)>,
    pub waking_until: f64,
    pub last_idle: f64,
    pub hovered: bool,
    pub worried: bool,
    age: f32,
    eyes: [Eye; 2],
    gaze: [f32; 2],
    y: f32,
    mouth:Mouth,
    blush:f32,
}

impl Default for Pet {
    fn default() -> Self {
        Self { state: PetState::Awake, preview:PetState::Happy, reaction: None, waking_until: 0.0, last_idle: 0.0,
            hovered: false, worried: false, age: 0.0, eyes: [Eye::open(4.4); 2], gaze: [0.0; 2], y: 0.0, mouth:Mouth::target(PetState::Awake),blush:0.0 }
    }
}

impl Pet {
    pub fn react(&mut self, state: PetState) { self.reaction = Some((state, 2.6)); self.age = 0.0; }
    /// 反馈只在实际可见时计时，避免用户还在面板里操作时就播完。
    pub fn advance(&mut self, dt: f32, visible: bool) {
        if visible {
            if let Some((_, remaining)) = &mut self.reaction {
                *remaining -= dt;
                if *remaining <= 0.0 { self.reaction = None; }
            }
        }
    }
    pub fn music_pose(&self)->MusicPose {
        music_pose_at(self.age)
    }

    pub fn step(&mut self, state: PetState, t: f64, dt: f32, gaze: [f32; 2], visible: bool) {
        if state != self.state { self.state = state; self.age = 0.0; }
        if visible { self.age += dt; }
        let age = self.age;
        let mut eyes = [Eye::open(4.4); 2];
        let mut y = (t as f32 * 1.7).sin() * 0.35;
        let mut look = gaze;
        match state {
            PetState::Awake => {},
            PetState::Curious => {
                eyes[0].height = 5.6; eyes[1].height = 6.3;
                eyes[0].y = 0.6; eyes[1].y = -0.6;
            },
            PetState::Waking => {
                eyes[0].height = lerp(0.0, 4.4, smoothstep(0.1, 0.65, age));
                eyes[1].height = lerp(0.0, 4.4, smoothstep(0.4, 1.1, age));
                y = 1.0 - smoothstep(0.0, 1.2, age); look = [0.0; 2];
            },
            PetState::Happy => {
                eyes = [Eye::arc(2.6, -1.8); 2];
                y = -(age * 7.5).sin().abs() * 2.0 * (-age * 0.65).exp();
            },
            PetState::Satisfied => {
                eyes = [Eye::arc(2.2, -1.2); 2];
                y = (age * 8.0).sin() * 1.2 * (-age * 0.8).exp();
            },
            PetState::Music => {
                let sway=(age*1.4).sin();
                look=[0.0,0.0];y=-0.5+sway*0.25;
                match self.music_pose() {
                    MusicPose::Microphone=>{eyes=[Eye::arc(2.5,0.7);2];},
                    MusicPose::SideSing=>{eyes=[Eye::open(1.6);2];eyes[0].y=-0.5;eyes[1].y=0.5;look=[0.4,0.0];y=sway*0.35;},
                    MusicPose::HighNote=>{eyes=[Eye::arc(2.3,-0.7);2];eyes[0].tilt=-0.3;eyes[1].tilt=0.3;y=-1.5+sway*0.2;},
                    MusicPose::Hum=>{eyes=[Eye::arc(2.5,0.7);2];y=sway*0.3;},
                    MusicPose::Squint=>{eyes=[Eye::arc(1.6,0.0);2];y=sway*0.45;},
                    MusicPose::Smile=>{eyes=[Eye::arc(2.3,-0.8);2];eyes[0].y=-0.5;eyes[1].y=0.5;look=[sway*0.5,0.0];},
                }
            },
            PetState::Worried => {
                eyes[0] = Eye { spread: -0.6, tilt: -0.7, y: 0.4, ..Eye::open(2.8) };
                eyes[1] = Eye { spread: 0.6, tilt: 0.7, y: 1.0, ..Eye::open(1.8) };
            },
            PetState::Focused => { eyes = [Eye::open(1.8); 2]; look = [gaze[0] * 0.2, 0.0]; y = 0.0; },
            PetState::Dozing => { eyes = [Eye::open(0.6); 2]; y = 1.0; look = [0.0; 2]; },
            PetState::Sleeping => { eyes = [Eye::arc(2.4, 1.0); 2]; y = 0.8 + (t as f32 * 1.2).sin() * 0.4; look = [0.0; 2]; },
            PetState::Listening=>{eyes=[Eye::open(5.0);2];eyes[1].y=-0.4;look=[gaze[0]*0.4,0.0];},
            PetState::Thinking=>{eyes[0]=Eye::open(1.8);eyes[1]=Eye::open(4.6);look=[1.0,-0.7];y=0.0;},
            PetState::Loading=>{eyes=[Eye::open(3.8);2];look=[(age*1.2).sin()*0.7,0.0];y=0.0;},
            PetState::Excited=>{eyes=[Eye::open(6.2);2];y=-(age*5.0).sin().abs()*1.2;look=[0.0;2];},
            PetState::Laughing=>{eyes=[Eye::arc(2.8,-1.8);2];y=(age*8.0).sin()*0.6;look=[0.0;2];},
            PetState::Shy=>{eyes=[Eye::arc(2.0,0.8);2];look=[0.5,0.7];y=0.4;},
            PetState::Sad=>{eyes=[Eye::open(2.5);2];eyes[0].tilt=0.7;eyes[1].tilt=-0.7;y=0.8;look=[0.0,0.4];},
            PetState::Angry=>{eyes=[Eye::open(2.8);2];eyes[0].tilt=-0.7;eyes[1].tilt=0.7;y=-0.3;look=[0.0;2];},
            PetState::Surprised=>{eyes=[Eye::open(6.8);2];look=[0.0;2];y=-0.4;},
            PetState::Confused=>{eyes[0]=Eye::open(5.8);eyes[1]=Eye::open(2.6);eyes[1].y=0.5;look=[-0.5,0.0];},
        }
        if matches!(state, PetState::Awake | PetState::Curious | PetState::Focused | PetState::Worried) {
            let start = 2.0 + crate::anim::hash01((t / 5.6).floor() as u64) * 2.7;
            let blink = ((t % 5.6) as f32 - start) / 0.18;
            if (0.0..1.0).contains(&blink) {
                for eye in &mut eyes { eye.height *= 1.0 - (blink * std::f32::consts::PI).sin(); }
            }
        }
        let eye_dt=if state==PetState::Music {dt*0.2}else{dt};
        for (eye, goal) in self.eyes.iter_mut().zip(eyes) { eye.follow(goal, eye_dt); }
        for (axis, goal) in self.gaze.iter_mut().zip(look) { *axis = approach(*axis, goal, 10.0, dt); }
        self.y = approach(self.y, y, 14.0, dt);
        self.mouth.follow(Mouth::target(state),dt);
        self.blush=approach(self.blush,if matches!(state,PetState::Happy|PetState::Satisfied|PetState::Excited|PetState::Laughing|PetState::Shy){1.0}else{0.0},12.0,dt);
    }

    pub fn draw(&self, c: &Card, cx: f32, t: f64) {
        for (i, eye) in self.eyes.iter().enumerate() {
            let x = cx + if i == 0 { -5.6 } else { 5.6 } + self.gaze[0];
            let y = 14.5 + self.y + eye.y + self.gaze[1];
            let points: Vec<_> = (0..=12).map(|n| {
                let q = n as f32 / 6.0 - 1.0;
                (x + q * eye.spread, y + q * (eye.height / 2.0 + eye.tilt) + (1.0 - q*q) * eye.curve)
            }).collect();
            let stroke=if self.state==PetState::Music {3.6-0.8*smoothstep(0.5,1.5,eye.spread.abs())}else{3.6};
            c.path(&points, stroke, theme::WHITE);
            // 圆端点保持胶囊眼睛的轮廓。
            for p in [points[0], points[12]] { c.circle(p.0, p.1, stroke/2.0, theme::WHITE); }
        }
        if self.state!=PetState::Music {self.mouth.draw(c,cx+self.gaze[0]*0.5,22.0+self.y+self.gaze[1]*0.5);}
        if self.blush>0.01 {for x in [cx-10.5,cx+10.5]{c.fill(x-1.6,18.6+self.y,3.2,1.4,0.7,theme::RED_TOP.gamma_multiply(self.blush*0.6));}}
        match self.state {
            PetState::Dozing | PetState::Sleeping => {
                let bob = ((t as f32 * 1.4).sin() + 1.0) * 0.7;
                c.text(cx+14.0, 13.0-bob, "z", 7.0, W::Bold, Color32::from_rgb(170,160,255), H::Left);
                if self.state == PetState::Sleeping { c.text(cx+19.0, 9.0-bob, "z", 5.5, W::Bold, Color32::from_rgb(170,160,255), H::Left); }
            },
            PetState::Happy => {
                let k = smoothstep(0.0, 0.2, self.age) * (1.0-smoothstep(1.5, 2.5, self.age));
                let c = c.faded(k);
                for (dx, y, r) in [(-18.0, 9.0, 2.1), (18.0, 8.0, 2.8)] {
                    c.seg((cx+dx-r,y),(cx+dx+r,y),1.0,theme::GREEN);
                    c.seg((cx+dx,y-r),(cx+dx,y+r),1.0,theme::GREEN);
                }
            },
            PetState::Satisfied => crate::paint::droplet(c,cx+18.0,12.0,5.0,theme::CYAN),
            PetState::Music => {
                let alpha=smoothstep(0.0,1.0,self.age%MUSIC_PERIOD);
                if self.age>=MUSIC_PERIOD && alpha<1.0 {
                    self.draw_music(&c.faded(1.0-alpha),cx,music_pose_at(self.age-MUSIC_PERIOD));
                }
                self.draw_music(&c.faded(alpha),cx,self.music_pose());
            },
            PetState::Worried => {
                c.fill(cx+17.0,8.0,1.6,4.2,0.8,theme::YELLOW);
                c.circle(cx+17.8,14.5,0.9,theme::YELLOW);
            },
            PetState::Thinking|PetState::Loading=>{
                for i in 0..3 {let phase=(self.age*2.0-i as f32*0.7).sin()*0.5+0.5;c.circle(cx+13.0+i as f32*3.0,8.0,0.7+phase*0.25,theme::CYAN.gamma_multiply(0.3+phase*0.7));}
            },
            PetState::Excited=>{for (x,y) in [(cx-17.0,8.0),(cx+17.0,7.0)]{c.seg((x-1.7,y),(x+1.7,y),0.8,theme::YELLOW);c.seg((x,y-1.7),(x,y+1.7),0.8,theme::YELLOW);}},
            PetState::Sad=>crate::paint::droplet(c,cx+12.0,18.0+self.y,3.0,theme::CYAN),
            PetState::Angry=>{for side in [-1.0,1.0]{let x=cx+side*5.6;c.seg((x-2.2,8.0+self.y-side*0.7),(x+2.2,8.0+self.y+side*0.7),1.0,theme::WHITE);}},
            PetState::Surprised=>{c.seg((cx+15.0,7.0),(cx+16.5,5.0),0.9,theme::YELLOW);c.seg((cx+17.0,10.0),(cx+19.0,9.6),0.9,theme::YELLOW);},
            PetState::Confused=>{c.text(cx+14.0,10.0,"?",8.0,W::Semibold,theme::CYAN,H::Left);},
            _ => {},
        }
    }

    fn draw_music(&self,c:&Card,cx:f32,pose:MusicPose){
        let y=self.y+self.gaze[1];
        let pulse=(self.age*2.0).sin()*0.5+0.5;
        let mouth=|rx:f32,ry:f32|{
            let points:Vec<_>=(0..=24).map(|n|{let angle=n as f32/24.0*std::f32::consts::TAU;(cx-0.8+angle.cos()*rx,20.7+y+angle.sin()*ry)}).collect();
            c.path(&points,0.9,theme::WHITE);
        };
        match pose {
            MusicPose::Microphone=>mouth(1.1,0.8+pulse*0.5),
            MusicPose::SideSing=>mouth(1.5,0.8+pulse*0.3),
            MusicPose::HighNote=>mouth(1.7,1.6+pulse*0.5),
            MusicPose::Hum=>c.path(&[(cx-2.7,20.7+y),(cx-1.8,21.4+y),(cx-0.8,21.0+y),(cx+0.2,21.4+y),(cx+1.1,20.7+y)],0.9,theme::WHITE),
            MusicPose::Squint=>mouth(1.2,0.4+pulse*0.6),
            MusicPose::Smile=>c.path(&[(cx-2.5,20.6+y),(cx-1.6,21.6+y),(cx-0.5,21.9+y),(cx+0.6,21.6+y),(cx+1.5,20.6+y)],1.0,theme::WHITE),
        }
        if matches!(pose,MusicPose::Microphone|MusicPose::SideSing|MusicPose::HighNote) {
            microphone(c,cx,y,self.age);
        } else {
            let y=8.5+(self.age*1.4).sin()*0.6;
            c.seg((cx+16.0,y-3.0),(cx+16.0,y+3.0),1.1,theme::WHITE);
            c.seg((cx+16.0,y-3.0),(cx+19.0,y-2.0),1.1,theme::WHITE);
            c.circle(cx+14.8,y+3.0,1.5,theme::WHITE);
        }
    }
}
