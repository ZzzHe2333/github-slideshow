#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() { eprintln!("Windows 10/11 only."); }

#[cfg(windows)]
mod app {
    use std::ffi::{c_void, OsStr};
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::process::CommandExt;
    use std::ptr::{null, null_mut};
    use std::mem::zeroed;
    use std::process::Command;

    type H = *mut c_void;
    const WS_OVERLAPPEDWINDOW: u32 = 0x00CF0000;
    const WS_CHILD: u32 = 0x40000000;
    const WS_VISIBLE: u32 = 0x10000000;
    const WS_VSCROLL: u32 = 0x00200000;
    const CBS_DROPDOWNLIST: u32 = 3;
    const WM_CREATE: u32 = 1;
    const WM_DESTROY: u32 = 2;
    const WM_COMMAND: u32 = 0x0111;
    const WM_SETFONT: u32 = 0x0030;
    const CB_ADDSTRING: u32 = 0x0143;
    const CB_RESETCONTENT: u32 = 0x014b;
    const CB_SETCURSEL: u32 = 0x014e;
    const CB_GETCURSEL: u32 = 0x0147;
    const MB_OKCANCEL: u32 = 1;
    const MB_ICONWARNING: u32 = 0x30;
    const MB_ICONERROR: u32 = 0x10;
    const MB_ICONINFO: u32 = 0x40;
    const SW_SHOW: i32 = 5;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    const ID_COMBO: usize = 1001;
    const ID_REFRESH: usize = 1002;
    const ID_SWITCH: usize = 1003;

    #[repr(C)] struct Point { x: i32, y: i32 }
    #[repr(C)] struct Msg {
        hwnd: H, message: u32, wparam: usize, lparam: isize,
        time: u32, pt: Point, lprivate: u32
    }
    #[repr(C)] struct WndClass {
        style: u32,
        callback: Option<unsafe extern "system" fn(H,u32,usize,isize)->isize>,
        cls_extra: i32, wnd_extra: i32, instance: H, icon: H,
        cursor: H, background: H, menu: *const u16, name: *const u16,
    }

    #[link(name = "user32")]
    extern "system" {
        fn RegisterClassW(class: *const WndClass) -> u16;
        fn CreateWindowExW(ex: u32, class: *const u16, title: *const u16, style: u32,
            x: i32,y: i32,w: i32,h: i32,parent: H, menu: H, instance: H,param: *mut c_void)->H;
        fn DefWindowProcW(hwnd: H, msg: u32, wp: usize, lp: isize) -> isize;
        fn GetMessageW(msg: *mut Msg, hwnd: H, filtermin: u32, filtermax: u32) -> i32;
        fn TranslateMessage(msg: *const Msg) -> i32;
        fn DispatchMessageW(msg: *const Msg) -> isize;
        fn ShowWindow(hwnd: H, command: i32) -> i32;
        fn UpdateWindow(hwnd: H) -> i32;
        fn PostQuitMessage(code: i32);
        fn SetWindowTextW(hwnd: H, txt: *const u16) -> i32;
        fn SendMessageW(hwnd: H, msg: u32, wp: usize, lp: isize) -> isize;
        fn MessageBoxW(hwnd: H, msg: *const u16, title: *const u16, typ: u32) -> i32;
        fn LoadCursorW(instance: H, id: *const u16) -> H;
        fn EnableWindow(hwnd: H, enabled: i32) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" { fn GetModuleHandleW(module: *const u16) -> H; }
    #[link(name = "shell32")]
    extern "system" {
        fn IsUserAnAdmin() -> i32;
        fn ShellExecuteW(hwnd: H, op: *const u16, file: *const u16,
            params: *const u16, dir: *const u16, show: i32) -> H;
    }
    #[link(name = "gdi32")]
    extern "system" {
        fn CreateFontW(height: i32,width: i32,esc: i32,ori: i32,weight: i32,
            italic: u32,under: u32,strike: u32,charset: u32,out: u32,clip: u32,
            quality: u32,pitch: u32,face: *const u16) -> H;
        fn DeleteObject(obj: H) -> i32;
    }

    fn w(s: &str) -> Vec<u16> { OsStr::new(s).encode_wide().chain(Some(0)).collect() }
    unsafe fn set(h: H, s: &str) { SetWindowTextW(h,w(s).as_ptr()); }
    unsafe fn boxmsg(parent: H, s: &str, flags: u32) -> i32 {
        MessageBoxW(parent,w(s).as_ptr(),w("有线 IP 切换器").as_ptr(),flags)
    }
    fn hex(b: u8) -> Option<u8> {
        match b { b'0'..=b'9' => Some(b-b'0'),b'a'..=b'f'=>Some(b-b'a'+10),b'A'..=b'F'=>Some(b-b'A'+10), _ => None }
    }
    fn dec(s: &str) -> String {
        let bytes=s.as_bytes();let mut output=Vec::new();let mut i=0;
        while i<bytes.len() {
            if bytes[i]==b'%' && i+2<bytes.len() {
                if let (Some(a),Some(b))=(hex(bytes[i+1]),hex(bytes[i+2])) {
                    output.push(a*16+b);i+=3;continue;
                }
            }
            output.push(bytes[i]);i+=1;
        }
        String::from_utf8_lossy(&output).into_owned()
    }
    fn base64(b: &[u8]) -> String {
        const AL: &[u8;64]=b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s=String::new();
        for c in b.chunks(3) {
            let a=c[0];let b=*c.get(1).unwrap_or(&0);let d=*c.get(2).unwrap_or(&0);
            s.push(AL[(a>>2)as usize]as char);
            s.push(AL[(((a&3)<<4)|(b>>4))as usize]as char);
            s.push(if c.len()>1 {AL[(((b&15)<<2)|(d>>6))as usize]as char} else {'='});
            s.push(if c.len()>2 {AL[(d&63)as usize]as char} else {'='});
        }
        s
    }
    fn invoke(action: &str, idx: u32) -> Result<Vec<String>,String> {
        if action!="list" && action!="switch" {return Err("操作不受支持".into());}
        let script=format!("{}\ntry {{ Invoke-WiredSwitch -Action '{}' -Index {} }} catch {{ Write-Output ('ERROR|' + [Uri]::EscapeDataString($_.Exception.Message)); exit 2 }}\n",
            include_str!("network.ps1"),action,idx);
        let mut encoded=Vec::new();
        for c in script.encode_utf16() {encoded.extend_from_slice(&c.to_le_bytes());}
        let result=Command::new("powershell.exe")
            .args(["-NoLogo","-NoProfile","-NonInteractive","-ExecutionPolicy","Bypass",
                "-EncodedCommand",&base64(&encoded)])
            .creation_flags(CREATE_NO_WINDOW).output()
            .map_err(|e|format!("无法执行 PowerShell: {e}"))?;
        let lines:Vec<String>=String::from_utf8_lossy(&result.stdout)
            .lines().map(|x|x.trim().to_string()).filter(|x|!x.is_empty()).collect();
        if let Some(msg)=lines.iter().find(|x|x.starts_with("ERROR|")) {return Err(dec(&msg[6..]));}
        if !result.status.success() {return Err(format!("系统脚本失败，代码 {:?}，请检查管理员权限和网络管理组件。",result.status.code()));}
        Ok(lines)
    }

    #[derive(Clone)]
    struct Nic {idx:u32,name:String,ip:String,prefix:String,gw:String,dns:String,dhcp:String,status:String}
    struct State {hwnd:H,combo:H,details:H,note:H,button:H,font:H,rows:Vec<Nic>}
    static mut STATE:Option<State>=None;

    unsafe fn control(parent:H,class:&str,text:&str,style:u32,x:i32,y:i32,dx:i32,dy:i32,id:usize,font:H)->H {
        let h=CreateWindowExW(0,w(class).as_ptr(),w(text).as_ptr(),
            style,x,y,dx,dy,parent,id as H,GetModuleHandleW(null()),null_mut());
        if !font.is_null() {SendMessageW(h,WM_SETFONT,font as usize,1);}
        h
    }
    unsafe fn init(hwnd:H) {
        let font=CreateFontW(-19,0,0,0,400,0,0,0,1,0,0,5,0,w("Microsoft YaHei UI").as_ptr());
        control(hwnd,"STATIC","有线 IPv4 一键切换",WS_CHILD|WS_VISIBLE,28,21,570,34,0,font);
        control(hwnd,"STATIC","仅切换 192.168.0.192 ⇄ 192.168.0.193",WS_CHILD|WS_VISIBLE,28,66,570,28,0,font);
        control(hwnd,"STATIC","选择实体有线网卡：",WS_CHILD|WS_VISIBLE,28,112,300,26,0,font);
        let combo=control(hwnd,"COMBOBOX","",WS_CHILD|WS_VISIBLE|WS_VSCROLL|CBS_DROPDOWNLIST,
            28,143,460,240,ID_COMBO,font);
        control(hwnd,"BUTTON","刷新网卡",WS_CHILD|WS_VISIBLE,504,142,133,32,ID_REFRESH,font);
        let details=control(hwnd,"STATIC","正在获取网络配置…",WS_CHILD|WS_VISIBLE,
            28,202,610,167,0,font);
        let button=control(hwnd,"BUTTON","切换 IP 地址",WS_CHILD|WS_VISIBLE,
            28,380,609,42,ID_SWITCH,font);
        let note=control(hwnd,"STATIC","不主动修改网关、DNS、子网前缀或其他网卡。",
            WS_CHILD|WS_VISIBLE,28,440,610,54,0,font);
        STATE=Some(State{hwnd,combo,details,note,button,font,rows:Vec::new()});
    }
    unsafe fn selected() -> Option<Nic> {
        let s=STATE.as_ref()?;
        let index=SendMessageW(s.combo,CB_GETCURSEL,0,0);
        if index<0 {return None;}
        s.rows.get(index as usize).cloned()
    }
    unsafe fn update() {
        let s=match STATE.as_ref(){Some(x)=>x,None=>return};
        if let Some(n)=selected() {
            set(s.details,&format!("名称：{}\r\n当前 IP：{}\r\n子网前缀：/{}\r\n默认网关：{}\r\nIPv4 DNS：{}\r\nIP 分配：{}",
                n.name,n.ip,n.prefix,if n.gw.is_empty(){"未设置"}else{&n.gw},
                if n.dns.is_empty(){"未设置"}else{&n.dns},if n.dhcp=="Disabled"{"手动"}else{"DHCP / 未知"}));
            let okay=n.status=="Up" && n.dhcp=="Disabled" && (n.ip=="192.168.0.192"||n.ip=="192.168.0.193");
            EnableWindow(s.button,if okay{1}else{0});
            let note=if n.status!="Up"{"网卡没有连接，禁止切换。"}
                else if n.dhcp!="Disabled"{"DHCP 网卡不允许切换。"}
                else if n.ip=="MULTIPLE"{"当前网卡拥有多个 IPv4 地址，禁止切换。"}
                else if !okay{"当前地址不匹配 .192/.193，禁止切换。"}
                else if n.ip=="192.168.0.192"{"准备切换为 192.168.0.193。"}
                else {"准备切换为 192.168.0.192。"};
            set(s.note,note);
        } else {
            set(s.details,"没有可用的实体有线网卡。");
            set(s.note,"请检查网卡驱动或网线连接。");
            EnableWindow(s.button,0);
        }
    }
    unsafe fn reload() {
        let previous=selected().map(|x|x.idx);
        let res=invoke("list",0);
        let s=match STATE.as_mut(){Some(x)=>x,None=>return};
        s.rows.clear();
        SendMessageW(s.combo,CB_RESETCONTENT,0,0);
        match res {
            Ok(lines)=>{
                for line in lines {
                    let p:Vec<&str>=line.split('|').collect();
                    if p.len()!=9 || p[0]!="ADAPTER" {continue;}
                    if let Ok(idx)=p[1].parse::<u32>() {
                        s.rows.push(Nic{idx,name:dec(p[2]),ip:p[3].into(),prefix:p[4].trim().into(),
                            gw:dec(p[5]),dns:dec(p[6]),dhcp:p[7].into(),status:dec(p[8])});
                    }
                }
                for n in &s.rows {
                    let label=format!("{} · {} · {}",n.name,n.ip,n.status);
                    SendMessageW(s.combo,CB_ADDSTRING,0,w(&label).as_ptr() as isize);
                }
                if !s.rows.is_empty() {
                    let selected=s.rows.iter().position(|n|Some(n.idx)==previous)
                        .or_else(||s.rows.iter().position(|n|n.status=="Up")).unwrap_or(0);
                    SendMessageW(s.combo,CB_SETCURSEL,selected,0);
                }
                update();
            }
            Err(e)=>{set(s.note,&format!("读取错误：{e}"));EnableWindow(s.button,0);boxmsg(s.hwnd,&e,MB_ICONERROR);}
        }
    }
    unsafe fn switch_ip() {
        let n=match selected(){Some(x)=>x,None=>return};
        let target=if n.ip=="192.168.0.192"{"192.168.0.193"}else{"192.168.0.192"};
        let hwnd=STATE.as_ref().unwrap().hwnd;
        let prompt=format!("网卡：{}\r\n{} → {}\r\n\r\n只修改此实体有线网卡的 IPv4 地址记录。\r\n短暂断网属于正常现象。\r\n确定继续吗？",n.name,n.ip,target);
        if boxmsg(hwnd,&prompt,MB_OKCANCEL|MB_ICONWARNING)!=1{return;}
        if let Some(s)=STATE.as_ref(){EnableWindow(s.button,0);set(s.note,"正在检测并切换，请稍候…");UpdateWindow(hwnd);}
        match invoke("switch",n.idx) {
            Ok(lines)=>{
                if let Some(line)=lines.iter().find(|x|x.starts_with("SUCCESS|")){
                    let p:Vec<&str>=line.split('|').collect();
                    boxmsg(hwnd,&format!("切换完成：{} → {}",p.get(1).unwrap_or(&"?"),p.get(2).unwrap_or(&"?")),MB_ICONINFO);
                }else{boxmsg(hwnd,"系统没有报告成功。请检查网络设置。",MB_ICONWARNING);}
            }
            Err(e)=>{boxmsg(hwnd,&format!("切换未完成：\r\n{e}"),MB_ICONERROR);}
        }
        reload();
    }
    unsafe extern "system" fn proc(hwnd:H,msg:u32,wp:usize,lp:isize)->isize {
        match msg {
            WM_CREATE=>{init(hwnd);0}
            WM_COMMAND=>{
                let id=wp&0xffff;let notification=(wp>>16)&0xffff;
                if id==ID_COMBO && notification==1 {update();}
                else if id==ID_REFRESH && notification==0 {reload();}
                else if id==ID_SWITCH && notification==0 {switch_ip();}
                0
            }
            WM_DESTROY=>{
                if let Some(s)=STATE.take(){if !s.font.is_null(){DeleteObject(s.font);}}
                PostQuitMessage(0);0
            }
            _=>DefWindowProcW(hwnd,msg,wp,lp),
        }
    }
    pub fn run() {
        unsafe {
            if IsUserAnAdmin()==0 {
                if let Ok(path)=std::env::current_exe() {
                    let exe:Vec<u16>=path.as_os_str().encode_wide().chain(Some(0)).collect();
                    let result=ShellExecuteW(null_mut(),w("runas").as_ptr(),exe.as_ptr(),null(),null(),SW_SHOW);
                    if result as usize<=32 {boxmsg(null_mut(),"需要管理员权限才能修改本机静态 IP。",MB_ICONERROR);}
                }
                return;
            }
            let name=w("WiredIPSwitcherRustWindow");
            let instance=GetModuleHandleW(null());
            let class=WndClass{style:0,callback:Some(proc),cls_extra:0,wnd_extra:0,
                instance,icon:null_mut(),cursor:LoadCursorW(null_mut(),32512usize as *const u16),
                background:6usize as H,menu:null(),name:name.as_ptr()};
            if RegisterClassW(&class)==0 {boxmsg(null_mut(),"窗口初始化失败。",MB_ICONERROR);return;}
            let hwnd=CreateWindowExW(0,name.as_ptr(),w("Wired IP Switcher - 有线 IP 切换器").as_ptr(),
                WS_OVERLAPPEDWINDOW,260,170,685,545,null_mut(),null_mut(),instance,null_mut());
            if hwnd.is_null() {boxmsg(null_mut(),"无法创建窗口。",MB_ICONERROR);return;}
            ShowWindow(hwnd,SW_SHOW);UpdateWindow(hwnd);reload();
            let mut msg:Msg=zeroed();
            while GetMessageW(&mut msg,null_mut(),0,0)>0 {
                TranslateMessage(&msg);DispatchMessageW(&msg);
            }
        }
    }
}

#[cfg(windows)]
fn main(){app::run();}
