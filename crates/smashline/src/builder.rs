use crate::{AsHash40, Costume, ObjectEvent, Priority, StatusLine};

pub type AcmdFunction = unsafe extern "C" fn(&mut crate::L2CAgentBase);

pub unsafe extern "C" fn acmd_stub(_agent: &mut crate::L2CAgentBase) {}

pub type StateFunction<T> = unsafe extern "C" fn(&mut T);

mod __sealed {
    pub trait Sealed {}
}

pub trait StatusLineMarker: __sealed::Sealed {
    type Function<T>;
    type LineFunction<T>;
    const LINE: StatusLine;

    unsafe fn cast_function<T>(func: Self::Function<T>) -> *const ();
    unsafe fn cast_line_function<T>(func: Self::LineFunction<T>) -> *const ();

    unsafe fn cast_ptr<T>(ptr: *const ()) -> Self::Function<T>;
}

macro_rules! markers {
    ($($name:ident($($func:tt)*));*) => {
        $(
            pub struct $name;
            impl __sealed::Sealed for $name {}
            impl StatusLineMarker for $name {
                type Function<T> = $($func)* -> crate::L2CValue;
                type LineFunction<T> = $($func)*;
                const LINE: StatusLine = StatusLine::$name;

                unsafe fn cast_function<T>(func: Self::Function<T>) -> *const () {
                    func as *const ()
                }

                unsafe fn cast_line_function<T>(func: Self::LineFunction<T>) -> *const () {
                    func as *const ()
                }

                unsafe fn cast_ptr<T>(ptr: *const ()) -> Self::Function<T> {
                    std::mem::transmute(ptr)
                }
            }
        )*
    }
}

markers! {
    Pre(unsafe extern "C" fn(&mut T));
    Main(unsafe extern "C" fn(&mut T));
    End(unsafe extern "C" fn(&mut T));
    Init(unsafe extern "C" fn(&mut T));
    Exec(unsafe extern "C" fn(&mut T));
    ExecStop(unsafe extern "C" fn(&mut T));
    Post(unsafe extern "C" fn(&mut T));
    Exit(unsafe extern "C" fn(&mut T));
    MapCorrection(unsafe extern "C" fn(&mut T));
    FixCamera(unsafe extern "C" fn(&mut T));
    FixPosSlow(unsafe extern "C" fn(&mut T));
    CheckDamage(unsafe extern "C" fn(&mut T, &crate::L2CValue));
    CheckAttack(unsafe extern "C" fn(&mut T, &crate::L2CValue, &crate::L2CValue));
    OnChangeLr(unsafe extern "C" fn(&mut T, &crate::L2CValue, &crate::L2CValue));
    LeaveStop(unsafe extern "C" fn(&mut T, &crate::L2CValue, &crate::L2CValue));
    NotifyEventGimmick(unsafe extern "C" fn(&mut T, &crate::L2CValue));
    CalcParam(unsafe extern "C" fn(&mut T))
}

struct AcmdScript {
    category: crate::Acmd,
    replaces: crate::Hash40,
    function: AcmdFunction,
    priority: Priority
}

struct LineCallback {
    line: StatusLine,
    function: *const (),
}

struct StateCallback {
    event: ObjectEvent,
    function: *const (),
}

struct StatusScript {
    line: StatusLine,
    kind: i32,
    function: *const (),
}

pub struct Agent {
    kind_hash: crate::Hash40,
    costume: Costume,
    acmd: Vec<AcmdScript>,
    lines: Vec<LineCallback>,
    status: Vec<StatusScript>,
    events: Vec<StateCallback>,
}

impl Agent {
    pub fn new(agent: impl AsHash40) -> Self {
        Self {
            kind_hash: agent.as_hash40(),
            costume: Costume { min: -1, max: -1 },
            acmd: vec![],
            lines: vec![],
            status: vec![],
            events: vec![],
        }
    }

    pub fn set_costume(&mut self, costume: (i32, i32)) -> &mut Self {
        self.costume.min = costume.0;
        self.costume.max = costume.1;
        self
    }

    pub fn acmd(&mut self, name: &str, function: AcmdFunction, priority: Priority) -> &mut Self {
        let category = if name.starts_with("game") {
            Some(crate::Acmd::Game)
        }
        else if name.starts_with("effect") {
            Some(crate::Acmd::Effect)
        }
        else if name.starts_with("sound") {
            Some(crate::Acmd::Sound)
        }
        else if name.starts_with("expression") {
            Some(crate::Acmd::Expression)
        }
        else {
            None
        };
        if category.is_some() {
            self.acmd.push(AcmdScript {
                category: category.unwrap(),
                replaces: name.as_hash40(),
                function,
                priority,
            });
        }
        else {
            println!("ACMD Category for {} could not be found! Skipping...", name);
        }
        self
    }

    pub fn game_acmd(&mut self, name: impl AsHash40, function: AcmdFunction, priority: Priority) -> &mut Self {
        self.acmd.push(AcmdScript {
            category: crate::Acmd::Game,
            replaces: name.as_hash40(),
            function,
            priority
        });
        self
    }

    pub fn effect_acmd(&mut self, name: impl AsHash40, function: AcmdFunction, priority: Priority) -> &mut Self {
        self.acmd.push(AcmdScript {
            category: crate::Acmd::Effect,
            replaces: name.as_hash40(),
            function,
            priority
        });
        self
    }

    pub fn sound_acmd(&mut self, name: impl AsHash40, function: AcmdFunction, priority: Priority) -> &mut Self {
        self.acmd.push(AcmdScript {
            category: crate::Acmd::Sound,
            replaces: name.as_hash40(),
            function,
            priority
        });

        self
    }

    pub fn expression_acmd(&mut self, name: impl AsHash40, function: AcmdFunction, priority: Priority) -> &mut Self {
        self.acmd.push(AcmdScript {
            category: crate::Acmd::Expression,
            replaces: name.as_hash40(),
            function,
            priority
        });

        self
    }

    #[allow(unused_variables)]
    pub fn status<M: StatusLineMarker, T>(
        &mut self,
        line: M,
        kind: i32,
        function: M::Function<T>,
    ) -> &mut Self {
        self.status.push(StatusScript {
            line: M::LINE,
            kind,
            function: unsafe { M::cast_function(function) },
        });
        self
    }

    #[allow(unused)]
    pub fn on_line<M: StatusLineMarker, T>(
        &mut self,
        line: M,
        function: M::LineFunction<T>,
    ) -> &mut Self {
        self.lines.push(LineCallback {
            line: M::LINE,
            function: unsafe { M::cast_line_function(function) },
        });
        self
    }

    pub fn on_init<T>(&mut self, func: StateFunction<T>) -> &mut Self {
        self.events.push(StateCallback {
            event: ObjectEvent::Initialize,
            function: func as *const (),
        });

        self
    }

    pub fn on_fini<T>(&mut self, func: StateFunction<T>) -> &mut Self {
        self.events.push(StateCallback {
            event: ObjectEvent::Finalize,
            function: func as *const (),
        });

        self
    }

    pub fn on_start<T>(&mut self, func: StateFunction<T>) -> &mut Self {
        self.events.push(StateCallback {
            event: ObjectEvent::Start,
            function: func as *const (),
        });

        self
    }

    pub fn on_end<T>(&mut self, func: StateFunction<T>) -> &mut Self {
        self.events.push(StateCallback {
            event: ObjectEvent::End,
            function: func as *const (),
        });

        self
    }

    pub fn install(&self) {
        for acmd in self.acmd.iter() {
            crate::api::install_acmd_script_costume(
                self.kind_hash,
                self.costume,
                acmd.replaces,
                acmd.category,
                acmd.priority,
                acmd.function,
            );
        }

        for status in self.status.iter() {
            crate::api::install_status_script_costume(
                Some(self.kind_hash),
                self.costume,
                status.line,
                status.kind,
                status.function,
            );
        }

        for line in self.lines.iter() {
            crate::api::install_line_callback(Some(self.kind_hash), line.line, line.function);
        }

        for event in self.events.iter() {
            crate::api::install_state_callback(
                Some(self.kind_hash),
                event.event,
                event.function as *const (),
            );
        }
    }
}
