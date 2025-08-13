use locks::RwLock;
use skyline::hooks::InlineCtx;
use smash::lib::L2CValue;
use smashline::{Costume, Hash40, L2CFighterBase, StatusLine, Variadic};

pub type Callback = extern "C" fn(&mut L2CFighterBase);
pub type Callback1 = extern "C" fn(&mut L2CFighterBase, &mut L2CValue);
pub type Callback2 = extern "C" fn(&mut L2CFighterBase, &mut L2CValue, &mut L2CValue);

#[derive(Copy, Clone)]
pub enum StatusCallbackFunction {
    Pre(Callback),
    Main(Callback),
    End(Callback),
    Init(Callback),
    Exec(Callback),
    ExecStop(Callback),
    Post(Callback),
    Exit(Callback),
    MapCorrection(Callback),
    FixCamera(Callback),
    FixPosSlow(Callback),
    CheckDamage(Callback1),
    CheckAttack(Callback2),
    OnChangeLr(Callback2),
    LeaveStop(Callback2),
    NotifyEventGimmick(Callback1),
    CalcParam(Callback),
}

impl StatusCallbackFunction {
    pub fn as_address(&self) -> usize {
        match self {
            Self::Pre(func) => *func as *const () as usize,
            Self::Main(func) => *func as *const () as usize,
            Self::End(func) => *func as *const () as usize,
            Self::Init(func) => *func as *const () as usize,
            Self::Exec(func) => *func as *const () as usize,
            Self::ExecStop(func) => *func as *const () as usize,
            Self::Post(func) => *func as *const () as usize,
            Self::Exit(func) => *func as *const () as usize,
            Self::MapCorrection(func) => *func as *const () as usize,
            Self::FixCamera(func) => *func as *const () as usize,
            Self::FixPosSlow(func) => *func as *const () as usize,
            Self::CheckDamage(func) => *func as *const () as usize,
            Self::CheckAttack(func) => *func as *const () as usize,
            Self::OnChangeLr(func) => *func as *const () as usize,
            Self::LeaveStop(func) => *func as *const () as usize,
            Self::NotifyEventGimmick(func) => *func as *const () as usize,
            Self::CalcParam(func) => *func as *const () as usize,
        }
    }

    pub fn new(line: StatusLine, function: *const ()) -> Self {
        use StatusLine::*;
        match line {
            Pre => Self::Pre(unsafe { std::mem::transmute(function) }),
            Main => Self::Main(unsafe { std::mem::transmute(function) }),
            End => Self::End(unsafe { std::mem::transmute(function) }),
            Init => Self::Init(unsafe { std::mem::transmute(function) }),
            Exec => Self::Exec(unsafe { std::mem::transmute(function) }),
            ExecStop => Self::ExecStop(unsafe { std::mem::transmute(function) }),
            Post => Self::Post(unsafe { std::mem::transmute(function) }),
            Exit => Self::Exit(unsafe { std::mem::transmute(function) }),
            MapCorrection => Self::MapCorrection(unsafe { std::mem::transmute(function) }),
            FixCamera => Self::FixCamera(unsafe { std::mem::transmute(function) }),
            FixPosSlow => Self::FixPosSlow(unsafe { std::mem::transmute(function) }),
            CheckDamage => Self::CheckDamage(unsafe { std::mem::transmute(function) }),
            CheckAttack => Self::CheckAttack(unsafe { std::mem::transmute(function) }),
            OnChangeLr => Self::OnChangeLr(unsafe { std::mem::transmute(function) }),
            LeaveStop => Self::LeaveStop(unsafe { std::mem::transmute(function) }),
            NotifyEventGimmick => {
                Self::NotifyEventGimmick(unsafe { std::mem::transmute(function) })
            }
            CalcParam => Self::CalcParam(unsafe { std::mem::transmute(function) }),
            _ => unreachable!(),
        }
    }
}

pub struct StatusCallback {
    pub hash: Option<Hash40>,
    pub function: StatusCallbackFunction,
    pub costume: Costume,
}

pub static CALLBACKS: RwLock<Vec<StatusCallback>> = RwLock::new(Vec::new());

#[inline(always)]
fn call_callback(fighter: &mut L2CFighterBase, callback: Callback, _ctx: &InlineCtx) {
    callback(fighter);
}

#[inline(always)]
fn call_callback1(fighter: &mut L2CFighterBase, callback: Callback1, ctx: &InlineCtx) {
    let arg = unsafe { std::mem::transmute(ctx.registers[1].x()) };
    callback(fighter, arg);
}

#[inline(always)]
fn call_callback2(fighter: &mut L2CFighterBase, callback: Callback2, ctx: &InlineCtx) {
    let arg = unsafe { std::mem::transmute(ctx.registers[2].x()) };
    let arg2 = unsafe { std::mem::transmute(ctx.registers[3].x()) };
    callback(fighter, arg, arg2);
}

macro_rules! decl_functions {
    ($($name:ident($line:ident, $offset:expr, $code_cave:expr, $reg:expr) => $call_fn:ident);*) => {
        $(
            extern "C" fn $name(ctx: &InlineCtx) {
                let fighter: &'static mut L2CFighterBase =
                    unsafe { std::mem::transmute(ctx.registers[$reg].x()) };
                    
                let fighter = std::hint::black_box(fighter);

                let callbacks = crate::create_agent::status_callbacks(fighter);

                for callback in callbacks.iter() {
                    if let StatusCallbackFunction::$line(callback_fn) = *callback {
                       $call_fn(fighter, callback_fn, ctx);
                    }
                }
            }
        )*

        pub fn install_callback_hooks() {
            $(
                crate::nro_hook::add_hook($offset, $code_cave, $name);
            )*
            unsafe {
                crate::nro_hook::add_normal_hook(0x3250, &mut ORIGINAL, call_line_status_hook as *const ());
            }
        }
    }
}

decl_functions! {
    call_init_hook(Init, 0x1bb4, 0x1a100, 0) => call_callback;
    call_pre_hook(Pre, 0x2560, 0x16086c, 0) => call_callback;
    call_post_hook(Post, 0x31d4, 0x160884, 0) => call_callback;
    call_post_hook2(Post, 0x321c, 0x1608a0, 0) => call_callback;
    // call_main_hook(Main, 0x32a4, 0x1608bc, 0) => call_callback;
    // call_main_hook2(Main, 0x32ec, 0x1608d4, 0) => call_callback;
    call_fix_camera_hook(FixCamera, 0x35e4, 0x1608ec, 0) => call_callback;
    call_fix_camera_hook2(FixCamera, 0x362c, 0x160904, 0) => call_callback;
    cal_map_correction_hook(MapCorrection, 0x36b4, 0x211afc, 0) => call_callback;
    cal_map_correction_hook2(MapCorrection, 0x36fc, 0x211b14, 0) => call_callback;
    call_fix_pos_slow_hook(FixPosSlow, 0x3784, 0x211b2c, 0) => call_callback;
    call_fix_pos_slow_hook2(FixPosSlow, 0x37cc, 0x211b44, 0) => call_callback;
    call_end_hook(End, 0x689c, 0x211b5c, 0) => call_callback;
    call_exit_hook(Exit, 0x6950, 0x211b74, 0) => call_callback;
    call_exec_stop_hook(ExecStop, 0x70e8, 0x211b90, 0) => call_callback;
    call_exec_hook(Exec, 0x7134, 0x211ba8, 0) => call_callback;
    call_exec_stop_hook2(ExecStop, 0x1a020, 0x2118c4, 0) => call_callback;
    call_exec_hook2(Exec, 0x1a06c, 0x2118dc, 0) => call_callback;
    call_calc_param_hook(CalcParam, 0x1a2a8, 0x2118f4, 0) => call_callback;
    call_notify_event_gimmick_hook(NotifyEventGimmick, 0x1a434, 0x21190c, 0) => call_callback1;
    call_leave_stop_hook(LeaveStop, 0x1a5dc, 0x211924, 1) => call_callback2;
    call_on_change_lr_hook(OnChangeLr, 0x1a77c, 0x1a7b0, 1) => call_callback2;
    call_check_attack_hook(CheckAttack, 0x1ab30, 0x2aabf0, 1) => call_callback2;
    call_check_damage_hook(CheckDamage, 0x1b414, 0x2aac08, 0) => call_callback1
}

// main is handled differently so that we can call it after
static mut ORIGINAL: *const () = std::ptr::null();

unsafe fn call_line_status_hook(
    fighter: &mut L2CFighterBase,
    variadic: &mut Variadic,
    string: *const u8,
    va_list: u32,
) {
    let callable: extern "C" fn(&mut L2CFighterBase, &mut Variadic, *const u8, u32) =
        std::mem::transmute(ORIGINAL);
    callable(fighter, variadic, string, va_list);

    let callbacks = crate::create_agent::status_callbacks(fighter);
    for callback in callbacks.iter() {
        if let StatusCallbackFunction::Main(callback_fn) = *callback {
            callback_fn(fighter);
        }
    }
}
