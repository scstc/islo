//! 显式翻译界面文案，不转换邮件、待办或用户输入。
use std::{collections::HashMap,sync::OnceLock};

#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub enum Language {System,#[default] Chinese,English}

#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub enum Locale {#[default] ZhCn,En}

fn english()->&'static HashMap<String,String>{
    static EN:OnceLock<HashMap<String,String>>=OnceLock::new();
    EN.get_or_init(||serde_json::from_str(include_str!("../assets/locales/en.json")).expect("bundled English translations"))
}

impl Language {
    pub const ALL:[Self;3]=[Self::System,Self::Chinese,Self::English];
    pub fn key(self)->&'static str {match self {Self::System=>"system",Self::Chinese=>"zh-CN",Self::English=>"en"}}
    pub fn parse(key:&str)->Self {match key {"system"|"auto"=>Self::System,"en"|"en-US"=>Self::English,_=>Self::Chinese}}
    pub fn resolve(self)->Locale {match self {Self::System=>system_locale(),Self::Chinese=>Locale::ZhCn,Self::English=>Locale::En}}
    pub fn label(self,locale:Locale)->&'static str {match self {Self::System=>locale.text("跟随系统"),Self::Chinese=>"简体中文",Self::English=>"English"}}
}

impl Locale {
    pub fn text<'a>(self,source:&'a str)->&'a str {
        if self==Self::ZhCn{return source;}
        english().get(source).map(String::as_str).unwrap_or(source)
    }
    pub fn is_english(self)->bool {self==Self::En}
    /// Named placeholders are substituted once, so inserted user content is never translated.
    pub fn format(self,source:&str,values:&[(&str,String)])->String {
        let template=self.text(source);
        let mut result=String::new();let mut rest=template;
        while let Some(start)=rest.find('{') {
            result.push_str(&rest[..start]);
            let Some(end)=rest[start..].find('}') else{result.push_str(&rest[start..]);return result;};
            let key=&rest[start+1..start+end];
            if let Some((_,value))=values.iter().find(|(name,_)|*name==key){result.push_str(value);}
            else{result.push_str(&rest[start..=start+end]);}
            rest=&rest[start+end+1..];
        }
        result.push_str(rest);result
    }
    /// Only for application-generated status/error strings. Never pass mail, task titles,
    /// file names, account aliases or external notification content to this method.
    /// Backend messages remain in their source language so switching languages also updates
    /// already-received statuses without reconnecting accounts or changing stored data.
    pub fn status(self,source:&str)->String {
        self.status_depth(source,0)
    }
    fn status_depth(self,source:&str,depth:usize)->String {
        if self==Self::ZhCn{return source.into();}
        let exact=self.text(source);if exact!=source{return exact.into();}
        if !source.chars().any(|ch|('\u{4e00}'..='\u{9fff}').contains(&ch)){return source.into();}
        static PATTERNS:OnceLock<Vec<(String,regex::Regex,Vec<String>)>>=OnceLock::new();
        let patterns=PATTERNS.get_or_init(||{
            let fields=regex::Regex::new(r"\{([0-9]+)\}").unwrap();
            let mut templates:Vec<_>=english().keys().filter(|key|fields.is_match(key)).cloned().collect();
            templates.sort_by(|a,b|b.len().cmp(&a.len()).then(a.cmp(b)));
            templates.into_iter().map(|template|{
                let mut pattern=String::from("(?s)^");let mut end=0;let mut names=vec![];
                for field in fields.captures_iter(&template){let span=field.get(0).unwrap();pattern.push_str(&regex::escape(&template[end..span.start()]));pattern.push_str("(.*?)");names.push(field[1].into());end=span.end();}
                pattern.push_str(&regex::escape(&template[end..]));pattern.push('$');
                (template,regex::Regex::new(&pattern).unwrap(),names)
            }).collect()
        });
        for (template,pattern,names) in patterns {
            if let Some(captures)=pattern.captures(source){
                // These fields contain generated status/error details, rather than user content.
                let detail=match template.as_str(){"{0}：{1}"|"正在监听 Windows 通知 · 当前可见 {0} 条\n{1}"=>Some("1"),"需要授权码：{0}"|"需要授权：{0}"|"连接失败：{0}"|"仅监听 QQ · {0}"|"{0} · 10 秒后重试"=>Some("0"),_=>None};
                let values:Vec<_>=names.iter().enumerate().map(|(i,name)|(name.as_str(),if depth<3&&detail==Some(name.as_str()){self.status_depth(&captures[i+1],depth+1)}else{captures[i+1].to_owned()})).collect();
                return self.format(template,&values);
            }
        }
        source.into()
    }
    pub fn repeat(self,mask:u8,short:bool)->String {
        if self==Self::ZhCn{return if short{crate::model::repeat_tag(mask)}else{crate::model::repeat_label(mask)};}
        match mask & crate::model::REPEAT_ALL {
            0=>if short{""}else{"Never"}.into(),
            crate::model::REPEAT_ALL=>"Every day".into(),
            crate::model::REPEAT_WORKDAYS=>"Weekdays".into(),
            crate::model::REPEAT_WEEKEND=>"Weekends".into(),
            mask=>{let days=(0..7).filter(|day|mask&(1<<day)!=0).map(|day|self.weekday(day)).collect::<Vec<_>>().join(", ");if short{days}else{format!("Every {days}")}},
        }
    }
    pub fn weekday(self,monday_index:usize)->&'static str {
        (match self {Self::ZhCn=>["周一","周二","周三","周四","周五","周六","周日"],Self::En=>["Mon","Tue","Wed","Thu","Fri","Sat","Sun"]})[monday_index%7]
    }
}

/// Helpers without an App reference read the owning egui context, never a process-global language.
pub fn set_locale(ctx:&eframe::egui::Context,locale:Locale){ctx.data_mut(|data|data.insert_temp(eframe::egui::Id::new("islo-locale"),locale));}
pub fn locale(ctx:&eframe::egui::Context)->Locale{ctx.data(|data|data.get_temp(eframe::egui::Id::new("islo-locale"))).unwrap_or(Locale::ZhCn)}

pub fn locale_from_tag(tag:&str)->Locale {
    if tag.trim().split(['-','_','.']).next().is_some_and(|language|language.eq_ignore_ascii_case("zh")){Locale::ZhCn}else{Locale::En}
}

fn system_locale()->Locale {
    #[cfg(windows)] {
        // 使用显示语言，避免把地区或日期格式误认为界面语言。
        let language=unsafe{windows::Win32::Globalization::GetUserDefaultUILanguage()};
        if language & 0x03ff==0x0004 {Locale::ZhCn}else{Locale::En}
    }
    #[cfg(not(windows))] {
        let language=["LC_ALL","LC_MESSAGES","LANG"].into_iter().find_map(|key|std::env::var(key).ok().filter(|value|!value.is_empty()));
        locale_from_tag(language.as_deref().unwrap_or("en"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formatted_text_preserves_inserted_user_values_and_placeholder_syntax(){
        let account="世界时钟 {1} <work@example.com>";
        assert_eq!(Locale::En.format("收件邮箱：{0}",&[("0",account.into())]),format!("Account: {account}"));
        assert_eq!(Locale::ZhCn.format("收件邮箱：{0}",&[("0",account.into())]),format!("收件邮箱：{account}"));
        assert_eq!(Locale::En.status("当前电量 80%"),"Battery 80%");
        assert_eq!(Locale::En.status("work@example.com：正在测试连接…"),"work@example.com: Testing connection…");
        assert_eq!(Locale::En.status("Unknown server error"),"Unknown server error");
        assert_eq!(Locale::En.repeat(crate::model::REPEAT_WORKDAYS,false),"Weekdays");
        assert_eq!(Locale::En.repeat(5,false),"Every Mon, Wed");
    }
    #[test]
    fn translations_keep_placeholder_names_and_have_no_untranslated_chinese(){
        let fields=regex::Regex::new(r"\{([0-9]+)\}").unwrap();
        for (source,translation) in english(){
            let names=|text:&str|{let mut names=fields.captures_iter(text).map(|c|c[1].to_owned()).collect::<Vec<_>>();names.sort();names};
            assert_eq!(names(source),names(translation),"Translation changed placeholders: {source}");
            assert!(!translation.chars().any(|ch|('\u{4e00}'..='\u{9fff}').contains(&ch)),"Untranslated catalog value: {source}");
        }
    }
    #[test]
    fn language_preferences_round_trip_and_keep_legacy_chinese_default() {
        for language in Language::ALL {assert_eq!(Language::parse(language.key()),language);}
        assert_eq!(Language::parse("invalid"),Language::Chinese);
        assert_eq!(Language::default(),Language::Chinese);
        assert_eq!(Language::English.resolve(),Locale::En);
        assert_eq!(Language::Chinese.resolve(),Locale::ZhCn);
    }
    #[test]
    fn locale_detection_handles_regions_and_unsupported_languages() {
        for tag in ["zh-CN","zh_TW.UTF-8","ZH-Hans-CN"]{assert_eq!(locale_from_tag(tag),Locale::ZhCn);}
        for tag in ["en-US","fr-FR","", "ja_JP.UTF-8"]{assert_eq!(locale_from_tag(tag),Locale::En);}
    }
    #[test]
    fn explicit_translation_leaves_unrecognized_user_content_untouched() {
        assert_eq!(Locale::En.text("世界时钟"),"World clock");
        assert_eq!(Locale::ZhCn.text("世界时钟"),"世界时钟");
        assert_eq!(Locale::En.text("用户自己的会议名称"),"用户自己的会议名称");
    }
    #[test]
    fn clock_periods_follow_language_without_changing_time_format_preferences() {
        let mut settings=crate::model::Settings::default();
        settings.language=Language::English;settings.clock_12h=true;settings.clock_period=1;
        assert_eq!(settings.clock_text("00:00:00"),"12:00 AM");
        assert_eq!(settings.clock_text("12:00:00"),"12:00 PM");
        settings.clock_period=2;
        assert_eq!(settings.clock_text("09:00:00"),"9:00 Morning");
        assert_eq!(settings.clock_text("13:00:00"),"1:00 Afternoon");
        settings.clock_period=1;
        settings.language=Language::Chinese;
        assert_eq!(settings.clock_text("12:00:00"),"下午 12:00");
        assert!(settings.clock_12h);assert_eq!(settings.clock_period,1);
    }
}
