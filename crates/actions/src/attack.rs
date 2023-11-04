use acmd_engine::action::Action;
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

#[cfg(feature = "bevy_reflect")]
use bevy_reflect::prelude::*;

use hash40::{hash40, Hash40};
use smash::{app::lua_bind::AttackModule, phx::Hash40 as GameHash40};

use crate::{decl_action, SerdeHash40};

impl CollisionAttribute {
    pub const fn as_hash(&self) -> SerdeHash40 {
        match self {
            Self::Aura => SerdeHash40::new("collision_attr_aura"),
            Self::Bind => SerdeHash40::new("collision_attr_bind"),
            Self::BindExtra => SerdeHash40::new("collision_attr_bind_extra"),
            Self::BlasterThrowDown => SerdeHash40::new("collision_attr_blaster_throw_down"),
            Self::BlasterThrowUp => SerdeHash40::new("collision_attr_blaster_throw_up"),
            Self::Bury => SerdeHash40::new("collision_attr_bury"),
            Self::BuryR => SerdeHash40::new("collision_attr_bury_r"),
            Self::Coin => SerdeHash40::new("collision_attr_coin"),
            Self::CursePoison => SerdeHash40::new("collision_attr_curse_poison"),
            Self::Cutup => SerdeHash40::new("collision_attr_cutup"),
            Self::CutupMetal => SerdeHash40::new("collision_attr_cutup_metal"),
            Self::Death => SerdeHash40::new("collision_attr_death"),
            Self::DeathBall => SerdeHash40::new("collision_attr_deathball"),
            Self::DededeHammer => SerdeHash40::new("collision_attr_dedede_hammer"),
            Self::Elec => SerdeHash40::new("collision_attr_elec"),
            Self::ElecWhip => SerdeHash40::new("collision_attr_elec_whip"),
            Self::Fire => SerdeHash40::new("collision_attr_fire"),
            Self::Flower => SerdeHash40::new("collision_attr_flower"),
            Self::Ice => SerdeHash40::new("collision_attr_ice"),
            Self::InkHit => SerdeHash40::new("collision_attr_ink_hit"),
            Self::JackBullet => SerdeHash40::new("collision_attr_jack_bullet"),
            Self::JackFinal => SerdeHash40::new("collision_attr_jack_final"),
            Self::Lay => SerdeHash40::new("collision_attr_lay"),
            Self::LeviathanWave => SerdeHash40::new("collision_attr_leviathan_wave"),
            Self::LeviathanWaveOwner => SerdeHash40::new("collision_attr_leviathan_wave_owner"),
            Self::Magic => SerdeHash40::new("collision_attr_magic"),
            Self::MarioLocalCoin => SerdeHash40::new("collision_attr_mario_local_coin"),
            Self::MarthShieldBreaker => SerdeHash40::new("collision_attr_marth_shield_breaker"),
            Self::Noamal => SerdeHash40::new("collision_attr_noamal"),
            Self::None => SerdeHash40::new("collision_attr_none"),
            Self::Normal => SerdeHash40::new("collision_attr_normal"),
            Self::NormalBullet => SerdeHash40::new("collision_attr_normal_bullet"),
            Self::OdinSlash => SerdeHash40::new("collision_attr_odin_slash"),
            Self::PalutenaBullet => SerdeHash40::new("collision_attr_palutena_bullet"),
            Self::Paralyze => SerdeHash40::new("collision_attr_paralyze"),
            Self::ParalyzeGhost => SerdeHash40::new("collision_attr_paralyze_ghost"),
            Self::Pierce => SerdeHash40::new("collision_attr_pierce"),
            Self::PitFall => SerdeHash40::new("collision_attr_pit_fall"),
            Self::Punch => SerdeHash40::new("collision_attr_punch"),
            Self::Purple => SerdeHash40::new("collision_attr_purple"),
            Self::Rush => SerdeHash40::new("collision_attr_rush"),
            Self::Saving => SerdeHash40::new("collision_attr_saving"),
            Self::SavingKen => SerdeHash40::new("collision_attr_saving_ken"),
            Self::Search => SerdeHash40::new("collision_attr_search"),
            Self::Sleep => SerdeHash40::new("collision_attr_sleep"),
            Self::SleepEx => SerdeHash40::new("collision_attr_sleep_ex"),
            Self::Slip => SerdeHash40::new("collision_attr_slip"),
            Self::Stab => SerdeHash40::new("collision_attr_stab"),
            Self::Sting => SerdeHash40::new("collision_attr_sting"),
            Self::StingBowArrow => SerdeHash40::new("collision_attr_sting_bowarrow"),
            Self::StingFlash => SerdeHash40::new("collision_attr_sting_flash"),
            Self::Stop => SerdeHash40::new("collision_attr_stop"),
            Self::TaiyoHit => SerdeHash40::new("collision_attr_taiyo_hit"),
            Self::Turn => SerdeHash40::new("collision_attr_turn"),
            Self::Water => SerdeHash40::new("collision_attr_water"),
            Self::Whip => SerdeHash40::new("collision_attr_whip"),
        }
    }
}

decl_action!(
    #[repr(i32)]
    #[derive(Copy, Default)]
    pub enum CollisionAttribute {
        Aura,
        Bind,
        BindExtra,
        BlasterThrowDown,
        BlasterThrowUp,
        Bury,
        BuryR,
        Coin,
        CursePoison,
        Cutup,
        CutupMetal,
        Death,
        DeathBall,
        DededeHammer,
        Elec,
        ElecWhip,
        Fire,
        Flower,
        Ice,
        InkHit,
        JackBullet,
        JackFinal,
        Lay,
        LeviathanWave,
        LeviathanWaveOwner,
        Magic,
        MarioLocalCoin,
        MarthShieldBreaker,
        Noamal,
        None,
        #[default]
        Normal,
        NormalBullet,
        OdinSlash,
        PalutenaBullet,
        Paralyze,
        ParalyzeGhost,
        Pierce,
        PitFall,
        Punch,
        Purple,
        Rush,
        Saving,
        SavingKen,
        Search,
        Sleep,
        SleepEx,
        Slip,
        Stab,
        Sting,
        StingBowArrow,
        StingFlash,
        Stop,
        TaiyoHit,
        Turn,
        Water,
        Whip,
    }
);

decl_action!(
    #[repr(i32)]
    #[derive(Copy, Default)]
    pub enum Sound {
        None = 0x0,
        #[default]
        Punch = 0x1,
        Kick = 0x2,
        CutUp = 0x3,
        Coin = 0x4,
        Bat = 0x5,
        Harisen = 0x6,
        Elec = 0x7,
        Fire = 0x8,
        Water = 0x9,
        Grass = 0xA,
        Bomb = 0xB,
        PeachWeapon = 0xC,
        Snake = 0xD,
        Ike = 0xE,
        Dedede = 0xF,
        Magic = 0x10,
        KameHit = 0x11,
        PeachBinta = 0x12,
        PeachFryingPan = 0x13,
        PeachGolf = 0x14,
        PeachTennis = 0x15,
        PeachParasol = 0x16,
        DaisyBinta = 0x17,
        DaisyFryingPan = 0x18,
        DaisyGolf = 0x19,
        DaisyTennis = 0x1A,
        DaisyParasol = 0x1B,
        Lucario = 0x1C,
        MarthSword = 0x1D,
        MarthFinal = 0x1E,
        MarioCoin = 0x1F,
        MarioFinal = 0x20,
        LuigiCoin = 0x21,
        NessBat = 0x22,
        Freeze = 0x23,
        MarioFireball = 0x24,
        MarioCoinLast = 0x25,
        MarioMant = 0x26,
        FoxBlaster = 0x27,
        LuigiAttackDash = 0x28,
        LuigiSmash = 0x29,
        MarioWaterPump = 0x2A,
        PacmanBell = 0x2B,
        GuruguruHit = 0x2C,
        LizardonFire = 0x2D,
        TrainHit = 0x2E,
        MarioDCoinLast = 0x2F,
        MarioDMant = 0x30,
        MarioDCapsule = 0x31,
        PacmanWater = 0x32,
        MiiGunnerBlaster = 0x33,
        RefletFinalSword = 0x34,
        RefletFinalFire = 0x35,
        RefletFinalElec = 0x36,
        DuckhuntFinal = 0x37,
        ShulkFinalDanban = 0x38,
        ShulkFinalRiki = 0x39,
        FalcoBlaster = 0x3A,
        RyuPunch = 0x3B,
        RyuKick = 0x3C,
        LucasBat = 0x3D,
        RyuFinal01 = 0x3E,
        RyuFinal02 = 0x3F,
        RyuFinal03 = 0x40,
        CloudHit = 0x41,
        CloudSmash01 = 0x42,
        CloudSmash02 = 0x43,
        CloudSmash03 = 0x44,
        CloudFinal01 = 0x45,
        CloudFinal02 = 0x46,
        CloudFinal03 = 0x47,
        BayonettaHit01 = 0x48,
        BayonettaHit02 = 0x49,
        YoshiBiteHit = 0x4A,
        YoshiEggHit = 0x4B,
        RoyHit = 0x4C,
        ChromHit = 0x4D,
        FoxTail = 0x4E,
        Heavy = 0x4F,
        Slap = 0x50,
        ItemHammer = 0x51,
        InklingHit = 0x52,
        MarioLocalCoin = 0x53,
        MarioLocalCoinLast = 0x54,
        FamicomHit = 0x55,
        ZenigameShellHit = 0x56,
        SamusScrew = 0x57,
        SamusDScrew = 0x58,
        SamusScrewFinish = 0x59,
        SamusDScrewFinish = 0x5A,
        KenPunch = 0x5B,
        KenKick = 0x5C,
        KenFinal01 = 0x5D,
        KenFinal02 = 0x5E,
        KenFinal03 = 0x5F,
        ShizueHammer = 0x60,
        SimonWhip = 0x61,
        SimonCross = 0x62,
        RichterWhip = 0x63,
        RichterCross = 0x64,
        SheikFinalPunch = 0x65,
        SheikFinalKick = 0x66,
        MetaknightFinalHit = 0x67,
        RobotFinalHit = 0x68,
        KenShoryu = 0x69,
        DiddyScratch = 0x6A,
        MiiEnemyGBlaster = 0x6B,
        ToonlinkHit = 0x6C,
        JackShot = 0x6D,
        BraveCriticalHit = 0x6E,
        BuddyWing = 0x6F,
        DollyPunch = 0x70,
        DollyKick = 0x71,
        DollyCritical = 0x72,
        DollySuperSpecial01 = 0x73,
        MasterAxe = 0x74,
        MasterArrowMax = 0x75,
        MasterAttack100End = 0x76,
        TantanPunch01 = 0x77,
        TantanPunch02 = 0x78,
        TantanPunch03 = 0x79,
        TantanFinal = 0x7A,
        CloudFinalAppendHit01 = 0x7B,
        CloudFinalAppendHit02 = 0x7C,
        FlameFinal = 0x7D,
        DemonPunch01 = 0x7E,
        DemonPunch02 = 0x7F,
        DemonKick = 0x80,
        DemonCatchAttack = 0x81,
        DemonFinal = 0x82,
        DemonThrowCommand = 0x83,
        DemonAttackSquat4 = 0x84,
        DemonAttackLw3 = 0x85,
        DemonAppeal = 0x86,
        TrailSlash = 0x87,
        TrailStab = 0x88,
        TrailCleave = 0x89,
        TrailCleaveSingle = 0x8A,
        TrailKick = 0x8B,
        TrailFinal = 0x8C,
    }
);

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone, Deserialize, Serialize)]
    #[cfg_attr(feature = "bevy_reflect", derive(bevy_reflect::Reflect))]
    #[cfg_attr(feature = "bevy_reflect", reflect_value)]
    pub struct SituationMask: u32 {
        const GROUND = 0x1;
        const AIR = 0x2;
        const ODD = 0x4;
        const IGNORE_DOWN = 0x80000000;
    }
}

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone, Deserialize, Serialize)]
    #[cfg_attr(feature = "bevy_reflect", derive(bevy_reflect::Reflect))]
    #[cfg_attr(feature = "bevy_reflect", reflect_value)]
    pub struct CategoryMask: u32 {
        const FIGHTER = 0x1;
        const ENEMY = 0x2;
        const ITEM = 0x4;
        const GIMMICK = 0x8;
        const ITEM_ENEMY = 0x10;
        const FLOOR = 0x20;
    }
}

bitflags::bitflags! {
    #[derive(Debug, Copy, Clone, Deserialize, Serialize)]
    #[cfg_attr(feature = "bevy_reflect", derive(bevy_reflect::Reflect))]
    #[cfg_attr(feature = "bevy_reflect", reflect_value)]
    pub struct PartMask: u32 {
        const BODY = 0x1;
        const ETC = 0x2;
        const LEGS = 0x4;
        const BODY_LEGS = 0x8;
        const HEAD = 0x10;
    }
}

decl_action!(
    #[derive(Copy, Default)]
    pub enum AttackAngle {
        Normal(i32),
        #[default]
        Sakurai,
        RadialOutward,
        AttackerVelocity,
        HalfAttackerVelocity,
        WeakAutolink,
        StrongAutolink,
        Vector {
            relative_to: SerdeHash40,
            offset: Vec2,
            num_frames: i32,
        },
    }
);

impl AttackAngle {
    fn as_angle(&self) -> i32 {
        match self {
            Self::Normal(angle) => (*angle).clamp(0, 360),
            Self::Sakurai => 361,
            Self::RadialOutward => 362,
            Self::AttackerVelocity => 363,
            Self::HalfAttackerVelocity => 365,
            Self::WeakAutolink => 366,
            Self::StrongAutolink => 367,
            Self::Vector { .. } => 368,
        }
    }
}

decl_action!(
    #[derive(Copy)]
    pub enum Knockback {
        Fixed(i32),
        Scaling { growth: i32, base: i32 },
    }
);

impl Default for Knockback {
    fn default() -> Self {
        Self::Scaling {
            growth: 30,
            base: 30,
        }
    }
}

decl_action!(
    #[derive(Copy)]
    pub enum Shape {
        Sphere(Vec3),
        Capsule { p1: Vec3, p2: Vec3 },
    }
);

impl Default for Shape {
    fn default() -> Self {
        Self::Sphere(Vec3::default())
    }
}

decl_action!(
    #[derive(Copy, Default)]
    pub enum ShieldSetoff {
        #[default]
        Off = 0,
        On,
        Through,
        NoStop,
    }
);

decl_action!(
    #[derive(Copy, Default)]
    pub enum HitDirection {
        #[default]
        CheckPos = 0,
        CheckSpeed,
        CheckLr,
        Forward,
        Backward,
        Part,
        BackSlash,
        Left,
        Right,
    }
);

decl_action!(
    #[derive(Copy)]
    #[serde(untagged)]
    pub enum ShieldDamage {
        Transcendent,
        AdditionalDamage(f32),
    }
);

impl Default for ShieldDamage {
    fn default() -> Self {
        Self::AdditionalDamage(0.0)
    }
}

decl_action!(
    #[repr(i32)]
    #[derive(Copy, Default)]
    pub enum SoundLevel {
        Small = 0,
        Medium,
        #[default]
        Large,
        ExtraLarge,
    }
);

decl_action!(
    #[repr(i32)]
    #[derive(Copy, Default)]
    pub enum AttackRegion {
        None = 0,
        Head,
        Body,
        Hip,
        #[default]
        Punch,
        Elbow,
        Kick,
        Knee,
        Throw,
        Object,
        Sword,
        Hammer,
        Bomb,
        Spin,
        Bite,
        Magic,
        PSI,
        Palutena,
        Aura,
        Bat,
        Parasol,
        Pikmin,
        Water,
        Whip,
        Tail,
        Energy,
    }
);

decl_action!(
    pub struct Attack {
        pub id: u64,
        pub part: u64,
        pub bone: SerdeHash40,
        pub damage: f32,
        pub angle: AttackAngle,
        pub knockback: Knockback,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub extra_histun: Option<f32>,
        pub radius: f32,
        pub shape: Shape,
        pub hitlag_mul: f32,
        pub sdi_strength_mul: f32,
        pub shield_pushback: ShieldSetoff,
        pub hit_direction: HitDirection,
        pub set_weight: bool,
        pub shield_damage: ShieldDamage,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub trip_chance: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub rehit_rate: Option<NonZeroU32>,
        pub reflectable: bool,
        pub absorbable: bool,
        pub flinchless: bool,
        pub disable_hitlag: bool,
        pub direct: bool,
        pub target_situations: SituationMask,
        pub target_categories: CategoryMask,
        pub target_parts: PartMask,
        pub friendly_fire: bool,
        pub collision_effect: CollisionAttribute,
        pub sound_level: SoundLevel,
        pub sound: Sound,
        pub region: AttackRegion,
    }
);

impl Default for Attack {
    fn default() -> Self {
        Self {
            id: 0,
            part: 0,
            bone: SerdeHash40::new("top"),
            damage: 15.0,
            angle: AttackAngle::Sakurai,
            knockback: Knockback::Scaling {
                growth: 50,
                base: 30,
            },
            extra_histun: None,
            radius: 5.0,
            shape: Shape::Sphere(Vec3::default()),
            hitlag_mul: 1.0,
            sdi_strength_mul: 1.0,
            shield_pushback: ShieldSetoff::On,
            hit_direction: HitDirection::CheckPos,
            set_weight: false,
            shield_damage: ShieldDamage::AdditionalDamage(0.0),
            trip_chance: None,
            rehit_rate: None,
            reflectable: false,
            absorbable: false,
            flinchless: false,
            disable_hitlag: false,
            direct: true,
            target_situations: SituationMask::all(),
            target_categories: CategoryMask::all(),
            target_parts: PartMask::all(),
            friendly_fire: false,
            collision_effect: CollisionAttribute::Normal,
            sound_level: SoundLevel::Medium,
            sound: Sound::Punch,
            region: AttackRegion::Punch,
        }
    }
}

impl Action for Attack {
    const NAME: &'static str = "Attack.set";

    #[cfg(feature = "bevy_reflect")]
    unsafe fn execute(&self, context: &mut smash::lua2cpp::L2CAgentBase) {
        unimplemented!()
    }

    #[cfg(not(feature = "bevy_reflect"))]
    unsafe fn execute(&self, context: &mut smash::lua2cpp::L2CAgentBase) {
        let (kbg, fkb, bkb) = match &self.knockback {
            Knockback::Fixed(value) => (0, *value, 0),
            Knockback::Scaling { growth, base } => (*growth, 0, *base),
        };

        let ([x, y, z], [x2, y2, z2]) = match &self.shape {
            Shape::Sphere(p) => ([p.x, p.y, p.z], [None, None, None]),
            Shape::Capsule { p1, p2 } => ([p1.x, p1.y, p1.z], [Some(p2.x), Some(p2.y), Some(p2.z)]),
        };

        smash_script::macros::ATTACK(
            context,
            self.id,
            self.part,
            GameHash40::new_raw(self.bone.0),
            self.damage,
            self.angle.as_angle() as u64,
            kbg,
            fkb,
            bkb,
            self.radius,
            x,
            y,
            z,
            x2,
            y2,
            z2,
            self.hitlag_mul,
            self.sdi_strength_mul,
            self.shield_pushback as i32,
            self.hit_direction as i32,
            self.set_weight,
            match &self.shield_damage {
                ShieldDamage::Transcendent => std::f32::NAN,
                ShieldDamage::AdditionalDamage(damage) => *damage,
            },
            self.trip_chance.unwrap_or(-1.0),
            self.rehit_rate.map(|rate| rate.get()).unwrap_or(0),
            self.reflectable,
            self.absorbable,
            self.flinchless,
            self.disable_hitlag,
            self.direct,
            self.target_situations.bits() as i32,
            self.target_categories.bits() as i32,
            self.target_parts.bits() as i32,
            self.friendly_fire,
            GameHash40::new_raw(self.collision_effect.as_hash().0),
            self.sound_level as i32,
            self.sound as i32,
            self.region as i32,
        );

        if let Some(extra_hitstun) = self.extra_histun {
            AttackModule::set_add_reaction_frame_revised(
                context.module_accessor,
                self.id as i32,
                extra_hitstun,
                false,
            );
        }

        if let AttackAngle::Vector {
            relative_to,
            offset,
            num_frames,
        } = &self.angle
        {
            AttackModule::set_vec_target_pos(
                context.module_accessor,
                self.id as i32,
                GameHash40::new_raw(relative_to.0),
                &smash::phx::Vector2f {
                    x: offset.x,
                    y: offset.y,
                },
                *num_frames as _,
                false,
            );
        }
    }
}

decl_action!(
    #[derive(Default)]
    struct AttackClear(i32);
);

impl Action for AttackClear {
    const NAME: &'static str = "Attack.clear";

    #[cfg(feature = "bevy_reflect")]
    unsafe fn execute(&self, context: &mut smash::lua2cpp::L2CAgentBase) {
        unimplemented!()
    }

    #[cfg(not(feature = "bevy_reflect"))]
    unsafe fn execute(&self, context: &mut smash::lua2cpp::L2CAgentBase) {
        AttackModule::clear(context.module_accessor, self.0, false);
    }
}

decl_action!(
    #[derive(Default)]
    struct AttackClearAll;
);
impl Action for AttackClearAll {
    const NAME: &'static str = "Attack.clear_all";

    #[cfg(feature = "bevy_reflect")]
    unsafe fn execute(&self, context: &mut smash::lua2cpp::L2CAgentBase) {
        unimplemented!()
    }

    #[cfg(not(feature = "bevy_reflect"))]
    unsafe fn execute(&self, context: &mut smash::lua2cpp::L2CAgentBase) {
        AttackModule::clear_all(context.module_accessor);
    }
}
