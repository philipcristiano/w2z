use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct InputFieldRequired(bool);
//impl TryFrom<bool> for InputFieldRequired {
//    type Error = &'static str;
//    fn try_from(b: bool) -> Result<InputFieldRequired, Self::Error> {
//        match b {
//            true => Ok(InputFieldRequired::Required),
//            false => Ok(InputFieldRequired::Optional),
//        }
//    }
//}

impl Default for InputFieldRequired {
    fn default() -> Self {
        InputFieldRequired(true)
    }
}

trait InputFieldImpl {
    fn markup(&self, prefix: &str, form_opts: FormInputOptions, blob: &Blob) -> maud::Markup;

    fn name(&self) -> &String;
    fn is_required(&self) -> &InputFieldRequired;

    fn field_name(&self, prefix: &str) -> String {
        format!("{}[{}]", prefix, self.name())
    }

    fn parse_value(
        &self,
        value: Option<&PostTypes>,
        context: DataContext,
    ) -> Result<Option<FieldValue>, TemplateError>;
}

trait InputFieldType: std::fmt::Debug + InputFieldImpl {}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct StringInputField {
    name: String,
    required: InputFieldRequired,
}

impl InputFieldType for StringInputField {}
impl InputFieldImpl for StringInputField {
    fn name(&self) -> &String {
        &self.name
    }
    fn is_required(&self) -> &InputFieldRequired {
        &self.required
    }

    fn markup(&self, prefix: &str, form_opts: FormInputOptions, blob: &Blob) -> maud::Markup {
        let field_name = self.field_name(prefix);
        maud::html! {
            @if form_opts.label == FormLabel::Yes { label  { (&field_name)} }
            @match form_opts.input_enable {
                InputEnable::Disabled =>{
                    input class="border min-w-full" name={(&field_name)} value={(blob.form_field_or_empty_string(&self.name))} disabled; {} }
                InputEnable::Enabled =>{
                    input class="border min-w-full" name={(&field_name)} value={(blob.form_field_or_empty_string(&self.name))} {}}

            }
        }
    }
    fn parse_value(
        &self,
        value: Option<&PostTypes>,
        context: DataContext,
    ) -> Result<Option<FieldValue>, TemplateError> {
        let value_s = if let Some(value) = value {
            let v = value.clone().value_string()?;
            if v.is_empty() {
                None
            } else {
                Some(v)
            }
        } else {
            None
        };

        match (value_s, &self.required) {
            (Some(v), _) => Ok(Some(FieldValue::String(v))),
            (None, InputFieldRequired(true)) => Err(TemplateError::MissingField {
                field: self.name.clone(),
            }),
            (None, InputFieldRequired(false)) => Ok(None),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct TextInputField {
    name: String,
    required: InputFieldRequired,
}

impl InputFieldType for TextInputField {}
impl InputFieldImpl for TextInputField {
    fn name(&self) -> &String {
        &self.name
    }
    fn is_required(&self) -> &InputFieldRequired {
        &self.required
    }
    fn markup(&self, prefix: &str, form_opts: FormInputOptions, blob: &Blob) -> maud::Markup {
        let field_name = self.field_name(prefix);
        maud::html! {
            @if form_opts.label == FormLabel::Yes { label  { (&field_name)} }
            textarea white-space="pre-wrap" class="border min-w-full" name={(&field_name)} {(blob.form_field_or_empty_string(self.name()))}
        }
    }
    fn parse_value(
        &self,
        value: Option<&PostTypes>,
        context: DataContext,
    ) -> Result<Option<FieldValue>, TemplateError> {
        let value_s = if let Some(value) = value {
            let v = value.clone().value_string()?;
            if v.is_empty() {
                None
            } else {
                Some(v)
            }
        } else {
            None
        };

        match (value_s, &self.required) {
            (Some(v), _) => Ok(Some(FieldValue::String(v))),
            (None, InputFieldRequired(true)) => Err(TemplateError::MissingField {
                field: self.name.clone(),
            }),
            (None, InputFieldRequired(false)) => Ok(None),
        }
    }
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct DateTimeInputField {
    name: String,
    default_now: bool,
    required: InputFieldRequired,
}
impl InputFieldType for DateTimeInputField {}
impl InputFieldImpl for DateTimeInputField {
    fn name(&self) -> &String {
        &self.name
    }
    fn is_required(&self) -> &InputFieldRequired {
        &self.required
    }
    fn markup(&self, prefix: &str, form_opts: FormInputOptions, blob: &Blob) -> maud::Markup {
        let field_name = self.field_name(prefix);
        maud::html! {
            label for={(&field_name)} { (&field_name)}
            span {"Date will be set automatically"}
        }
    }
    fn parse_value(
        &self,
        value: Option<&PostTypes>,
        context: DataContext,
    ) -> Result<Option<FieldValue>, TemplateError> {
        Ok(Some(FieldValue::DateTime(context.now)))
    }
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct ListInputField {
    name: String,
    required: InputFieldRequired,
}
impl InputFieldType for ListInputField {}
impl InputFieldImpl for ListInputField {
    fn name(&self) -> &String {
        &self.name
    }
    fn is_required(&self) -> &InputFieldRequired {
        &self.required
    }
    fn markup(&self, prefix: &str, form_opts: FormInputOptions, blob: &Blob) -> maud::Markup {
        let list_item_field_name = self.field_name(prefix);
        let item_template = InputField::String {
            name: "".to_string(),
            required: self.required.clone(),
        }
        .fieldimpl();
        maud::html! {
            span {"List items!"}
            br {}
            label { (&list_item_field_name)}
            button type="button" script="on click set N to the next <div/> then set N to N.cloneNode(true) then remove .hidden from N then remove @disabled from the <input/> in N then put N after me" {"Add item"}
            div class="hidden" {
                (item_template.markup(&list_item_field_name, form_opts.without_label().disable_input(), blob))
                button type="button" script="on click remove me.parentElement" {"Remove"}
            }
        }
    }

    fn parse_value(
        &self,
        value: Option<&PostTypes>,
        context: DataContext,
    ) -> Result<Option<FieldValue>, TemplateError> {
        if let Some(v) = value {
            Ok(Some(FieldValue::List(v.clone().value_strings()?)))
        } else {
            Ok(Some(FieldValue::List(vec![])))
        }
    }
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct ObjectInputField {
    name: String,
    input_fields: Vec<InputField>,
    required: InputFieldRequired,
}

impl InputFieldType for ObjectInputField {}
impl InputFieldImpl for ObjectInputField {
    fn name(&self) -> &String {
        &self.name
    }
    fn is_required(&self) -> &InputFieldRequired {
        &self.required
    }
    fn markup(&self, prefix: &str, form_opts: FormInputOptions, blob: &Blob) -> maud::Markup {
        let field_name = self.field_name(prefix);
        maud::html! {
            @if form_opts.label == FormLabel::Yes { label  { (field_name)} }
            @for if_field in self.input_fields.clone() {
                (if_field.fieldimpl().markup(&field_name, form_opts, blob))

            }
        }
    }

    fn parse_value(
        &self,
        value: Option<&PostTypes>,
        context: DataContext,
    ) -> Result<Option<FieldValue>, TemplateError> {
        if let Some(v) = value {
            let d = Blob::hm_to_valid_structure(
                v.clone().value_hm()?,
                self.input_fields.to_owned(),
                context,
            )?;
            Ok(Some(FieldValue::Object(d)))
        } else {
            Ok(None)
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum InputField {
    #[serde(alias = "string")]
    String {
        name: String,
        #[serde(default)]
        required: InputFieldRequired,
    },
    #[serde(alias = "text")]
    Text {
        name: String,
        #[serde(default)]
        required: InputFieldRequired,
    },
    #[serde(alias = "datetime")]
    DateTime {
        name: String,
        default_now: bool,
        #[serde(default)]
        required: InputFieldRequired,
    },
    #[serde(alias = "list")]
    List {
        name: String,
        // list_of_type: InputField,
        // Eventually support types, right now assume String type
        #[serde(default)]
        required: InputFieldRequired,
    },
    #[serde(alias = "object")]
    Object {
        name: String,
        input_fields: Vec<InputField>,
        #[serde(default)]
        required: InputFieldRequired,
    },
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub struct FormInputOptions {
    pub label: FormLabel,
    input_enable: InputEnable,
}

impl Default for FormInputOptions {
    fn default() -> FormInputOptions {
        FormInputOptions {
            label: FormLabel::Yes,
            input_enable: InputEnable::Enabled,
        }
    }
}

impl FormInputOptions {
    fn disable_input(self) -> Self {
        FormInputOptions {
            input_enable: InputEnable::Disabled,
            ..self
        }
    }

    fn without_label(self) -> Self {
        FormInputOptions {
            label: FormLabel::No,
            ..self
        }
    }
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum FormLabel {
    Yes,
    No,
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum InputEnable {
    Enabled,
    Disabled,
}

impl InputField {
    pub fn fieldimpl(self) -> Box<dyn InputFieldType> {
        let f: Box<dyn InputFieldType> = match self {
            InputField::String { name, required } => Box::new(StringInputField { name, required }),
            InputField::Text { name, required } => Box::new(TextInputField { name, required }),
            InputField::DateTime {
                name,
                default_now,
                required,
            } => Box::new(DateTimeInputField {
                name,
                default_now,
                required,
            }),
            InputField::List { name, required } => Box::new(ListInputField { name, required }),
            InputField::Object {
                name,
                input_fields,
                required,
            } => Box::new(ObjectInputField {
                name,
                input_fields,
                required,
            }),
        };
        return f;
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PostTypes {
    String(String),
    List(Vec<String>),
    Object(HashMap<String, PostTypes>),
}

impl PostTypes {
    fn value_string(self) -> Result<String, TemplateError> {
        match self {
            PostTypes::String(s) => Ok(s),
            PostTypes::List(_) => Err(TemplateError::FieldDataError(
                "Cannot convert to list".to_string(),
            )),
            PostTypes::Object(_) => Err(TemplateError::FieldDataError(
                "Cannot convert to string".to_string(),
            )),
        }
    }
    fn value_strings(self) -> Result<Vec<String>, TemplateError> {
        match self {
            PostTypes::String(_) => Err(TemplateError::FieldDataError(
                "Cannot convert to list".to_string(),
            )),
            PostTypes::List(l) => Ok(l),
            PostTypes::Object(_) => Err(TemplateError::FieldDataError(
                "Cannot convert to list".to_string(),
            )),
        }
    }

    fn value_hm(self) -> Result<HashMap<String, PostTypes>, TemplateError> {
        match self {
            PostTypes::Object(m) => Ok(m),
            PostTypes::List(_) => Err(TemplateError::FieldDataError(
                "Cannot convert to list".to_string(),
            )),
            PostTypes::String(_) => Err(TemplateError::FieldDataError(
                "Cannot convert to hashmap".to_string(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
struct DataContext {
    now: chrono::DateTime<chrono::Utc>,
}
impl Default for DataContext {
    fn default() -> Self {
        Self {
            now: chrono::Utc::now(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Blob {
    fields: HashMap<String, PostTypes>,
}

impl Blob {
    pub fn new() -> Blob {
        Blob {
            fields: HashMap::new(),
        }
    }

    pub fn form_field(&self, field_name: &str) -> Result<Option<String>, TemplateError> {
        if let Some(item) = self.fields.get(field_name) {
            Ok(Some(item.clone().value_string()?.clone()))
        } else {
            Ok(None)
        }
    }

    pub fn form_field_or_empty_string(&self, field_name: &str) -> String {
        if let Ok(Some(s)) = self.form_field(field_name) {
            s
        } else {
            "".to_string()
        }
    }

    fn to_valid_structure(
        self,
        input_fields: Vec<InputField>,
        context: DataContext,
    ) -> Result<indexmap::IndexMap<String, FieldValue>, TemplateError> {
        Blob::hm_to_valid_structure(self.fields, input_fields, context)
    }

    fn hm_to_valid_structure(
        hm: HashMap<String, PostTypes>,
        input_fields: Vec<InputField>,
        context: DataContext,
    ) -> Result<indexmap::IndexMap<String, FieldValue>, TemplateError> {
        let mut im = indexmap::IndexMap::new();
        for input_fieldw in input_fields {
            let input_field = input_fieldw.fieldimpl();
            tracing::info!(if = ?input_field, "field!");
            let maybe_blob_value = hm.get(input_field.name());
            if let Some(value) = input_field.parse_value(maybe_blob_value.to_owned(), context)? {
                im.insert(input_field.name().clone(), value);
            }
        }
        Ok(im)
    }
}

use thiserror::Error;
#[derive(Error, Debug, PartialEq, Eq)]
pub enum TemplateError {
    #[error("Field is missing: {field}")]
    MissingField { field: String },
    #[error("Error with data in a field")]
    FieldDataError(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FieldValue {
    #[serde(alias = "string")]
    String(String),
    #[serde(alias = "text")]
    Text(String),
    #[serde(alias = "list")]
    List(Vec<String>),
    #[serde(alias = "object")]
    Object(indexmap::IndexMap<String, FieldValue>),
    #[serde(alias = "datetime")]
    DateTime(chrono::DateTime<chrono::Utc>),
}

impl FieldValue {
    fn as_body_string(self) -> String {
        match self {
            FieldValue::String(s) => s,
            FieldValue::Text(t) => t,
            FieldValue::List(l) => "[list]".to_string(),
            FieldValue::Object(..) => "[object]".to_string(),
            FieldValue::DateTime(now) => now.to_string(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Template {
    input_fields: Vec<InputField>,
    pub path: String,
}

impl Template {
    pub fn config_messages(&self) -> Vec<String> {
        let mut m = vec![];

        if let Err(e) = self.renderer(&self.path) {
            m.push(format!("Could not parse path: {:?}", e));
        }
        m
    }

    fn renderer(&self, tmpl: &String) -> anyhow::Result<tera::Tera> {
        let mut t = tera::Tera::default();
        t.add_raw_template("tmpl", tmpl)?;
        Ok(t)
    }

    pub fn rendered_path(&self, data: Blob) -> anyhow::Result<String> {
        let d = data.to_valid_structure(self.input_fields.clone(), DataContext::default())?;
        let context = tera::Context::from_serialize(d)?;
        let r = self.renderer(&self.path)?;
        Ok(r.render("tmpl", &context)?)
    }

    pub fn form_fields_markup(
        &self,
        input_opts: FormInputOptions,
        input_form_data: &Blob,
    ) -> maud::Markup {
        let prefix = "fields".to_string();
        maud::html! {
            @for input_field in &self.input_fields {
                (input_field.clone().fieldimpl().markup(&prefix, input_opts, &input_form_data))
                br {}
            }

        }
    }

    pub fn as_toml(&self, data: Blob) -> Result<String, TemplateError> {
        tracing::info!(t= ?self, "template");
        let structured_data =
            data.to_valid_structure(self.input_fields.clone(), DataContext::default())?;

        let file_contents = format_toml_frontmatter_file(structured_data);
        Ok(file_contents)
    }
}

fn format_toml_frontmatter_file(mut data: indexmap::IndexMap<String, FieldValue>) -> String {
    let body = data.shift_remove("body");

    let fm = toml::to_string(&data).unwrap();
    if let Some(body) = body {
        let body_s = body.as_body_string();
        format!("+++\n{fm}+++\n\n{body_s}\n")
    } else {
        format!("+++\n{fm}+++\n")
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Deserialize)]
pub struct TestConfig {
    templates: indexmap::IndexMap<String, Template>,
}

#[cfg(test)]
mod test_template_load {

    use super::*;

    #[test]
    fn test_optional_object() {
        let cfg = r#"
[templates]
[templates.note]
input_fields = [{name = "object_name", type = "object", input_fields = [], required=false},
]
path = "/index.md"
"#;
        let config: crate::backends::github::SiteConfig =
            toml::from_str(cfg).expect("Parsed Config");
        let expected_obj = InputField::Object {
            name: "object_name".to_string(),
            input_fields: vec![],
            required: InputFieldRequired(false),
        };
        let t = config.get_template("note".to_string()).expect("Has note");

        assert_eq!(t.input_fields.first().expect("has one"), &expected_obj);
    }
}
#[cfg(test)]
mod test_structure {

    use super::*;

    #[test]
    fn single_text_field() {
        let text_field = InputField::Text {
            name: "title".to_string(),
            required: InputFieldRequired(true),
        };
        let mut fields_hm = HashMap::new();
        let val = PostTypes::String("Hello World!".to_string());
        fields_hm.insert("title".to_string(), val);

        let data = Blob { fields: fields_hm };

        let expected = r#"+++
title = "Hello World!"
+++
"#;

        let structured_data = data
            .to_valid_structure(vec![text_field], DataContext::default())
            .unwrap();

        let file_contents = format_toml_frontmatter_file(structured_data);

        assert_eq!(file_contents, expected);
    }

    #[test]
    fn single_text_field_missing_data() {
        let text_field = InputField::Text {
            name: "title".to_string(),
            required: InputFieldRequired(true),
        };
        let fields_hm = HashMap::new();
        let data = Blob { fields: fields_hm };

        let expected = TemplateError::MissingField {
            field: "title".to_string(),
        };
        let maybe_structured_data = data
            .to_valid_structure(vec![text_field], DataContext::default())
            .unwrap_err();
        assert_eq!(maybe_structured_data, expected);
    }

    #[test]
    fn single_text_field_empty_but_not_required() {
        let text_field = InputField::Text {
            name: "title".to_string(),
            required: InputFieldRequired(false),
        };
        let mut fields_hm = HashMap::new();
        let val = PostTypes::String("".to_string());
        fields_hm.insert("title".to_string(), val);
        let data = Blob { fields: fields_hm };
        let structured_data = data
            .to_valid_structure(vec![text_field], DataContext::default())
            .unwrap();

        assert_eq!(structured_data.keys().len(), 0);
    }

    #[test]
    fn single_text_field_missing_data_but_not_required() {
        let text_field = InputField::Text {
            name: "title".to_string(),
            required: InputFieldRequired(false),
        };
        let fields_hm = HashMap::new();
        let data = Blob { fields: fields_hm };
        let structured_data = data
            .to_valid_structure(vec![text_field], DataContext::default())
            .unwrap();

        assert_eq!(structured_data.keys().len(), 0);
    }

    #[test]
    fn single_text_field_empty_of_data() {
        let text_field = InputField::Text {
            name: "title".to_string(),
            required: InputFieldRequired(true),
        };
        let mut fields_hm = HashMap::new();
        let val = PostTypes::String("".to_string());
        fields_hm.insert("title".to_string(), val);
        let data = Blob { fields: fields_hm };

        let expected = TemplateError::MissingField {
            field: "title".to_string(),
        };
        let maybe_structured_data = data
            .to_valid_structure(vec![text_field], DataContext::default())
            .unwrap_err();
        assert_eq!(maybe_structured_data, expected);
    }

    #[test]
    fn single_string_field() {
        let text_field = InputField::String {
            name: "title".to_string(),
            required: InputFieldRequired(true),
        };
        let mut fields_hm = HashMap::new();
        let val = PostTypes::String("Hello World!".to_string());
        fields_hm.insert("title".to_string(), val);

        let data = Blob { fields: fields_hm };

        let expected = r#"+++
title = "Hello World!"
+++
"#;

        let structured_data = data
            .to_valid_structure(vec![text_field], DataContext::default())
            .unwrap();

        let file_contents = format_toml_frontmatter_file(structured_data);

        assert_eq!(file_contents, expected);
    }

    #[test]
    fn single_string_field_empty_of_data() {
        let field = InputField::String {
            name: "title".to_string(),
            required: InputFieldRequired(true),
        };
        let mut fields_hm = HashMap::new();
        let val = PostTypes::String("".to_string());
        fields_hm.insert("title".to_string(), val);
        let data = Blob { fields: fields_hm };

        let expected = TemplateError::MissingField {
            field: "title".to_string(),
        };
        let maybe_structured_data = data
            .to_valid_structure(vec![field], DataContext::default())
            .unwrap_err();
        assert_eq!(maybe_structured_data, expected);
    }

    #[test]
    fn multiple_strings() {
        let field = InputField::List {
            name: "items".to_string(),
            required: InputFieldRequired(true),
        };
        let mut fields_hm = HashMap::new();
        let val = PostTypes::List(vec!["1".to_string(), "2".to_string()]);
        fields_hm.insert("items".to_string(), val);
        let data = Blob { fields: fields_hm };

        let expected = r#"+++
items = ["1", "2"]
+++
"#;

        let structured_data = data
            .to_valid_structure(vec![field], DataContext::default())
            .unwrap();

        let file_contents = format_toml_frontmatter_file(structured_data);

        assert_eq!(file_contents, expected);
    }

    #[test]
    fn empty_list_of_strings_() {
        let field = InputField::List {
            name: "items".to_string(),
            required: InputFieldRequired(true),
        };
        let fields_hm = HashMap::new();
        let data = Blob { fields: fields_hm };

        let expected = r#"+++
items = []
+++
"#;

        let structured_data = data
            .to_valid_structure(vec![field], DataContext::default())
            .unwrap();

        let file_contents = format_toml_frontmatter_file(structured_data);

        assert_eq!(file_contents, expected);
    }

    #[test]
    fn single_datetime_field() {
        let dt_field = InputField::DateTime {
            name: "date".to_string(),
            default_now: true,
            required: InputFieldRequired(true),
        };
        let fields_hm = HashMap::new();

        let data = Blob { fields: fields_hm };
        let context = DataContext {
            now: chrono::Utc::now(),
        };

        // Format to match
        // https://github.com/pitdicker/chrono/blob/2d2062f576f306222217abb866feaa3dbfda94a2/src/datetime/serde.rs#L45
        // as precision can change with the default serde implementation depending on the platform
        let expected = format!(
            r#"+++
date = "{}"
+++
"#,
            context
                .now
                .to_rfc3339_opts(chrono::format::SecondsFormat::AutoSi, true)
        );

        let structured_data = data.to_valid_structure(vec![dt_field], context).unwrap();

        let file_contents = format_toml_frontmatter_file(structured_data);

        assert_eq!(file_contents, expected);
    }
    #[test]
    fn string_and_toml_content_field() {
        let string_field = InputField::String {
            name: "title".to_string(),
            required: InputFieldRequired(true),
        };
        let text_field = InputField::String {
            name: "body".to_string(),
            required: InputFieldRequired(true),
        };
        let mut fields_hm = HashMap::new();
        let sval = PostTypes::String("Hello World!".to_string());
        fields_hm.insert("title".to_string(), sval);
        let tval = PostTypes::String("Text body!".to_string());
        fields_hm.insert("body".to_string(), tval);

        let data = Blob { fields: fields_hm };

        let expected = r#"+++
title = "Hello World!"
+++

Text body!
"#;

        let structured_data = data
            .to_valid_structure(vec![string_field, text_field], DataContext::default())
            .unwrap();

        let file_contents = format_toml_frontmatter_file(structured_data);

        assert_eq!(file_contents, expected);
    }
    #[test]
    fn single_object_string_field() {
        let text_field = InputField::String {
            name: "title".to_string(),
            required: InputFieldRequired(true),
        };
        let obj_field = InputField::Object {
            name: "extra".to_string(),
            input_fields: vec![text_field],
            required: InputFieldRequired(true),
        };
        let val = PostTypes::String("Hello extra World!".to_string());

        let mut fields_hm = HashMap::new();
        fields_hm.insert("title".to_string(), val);
        let obj_val = PostTypes::Object(fields_hm);
        let mut obj_hm = HashMap::new();
        obj_hm.insert("extra".to_string(), obj_val);

        let data = Blob { fields: obj_hm };

        let expected = r#"+++
[extra]
title = "Hello extra World!"
+++
"#;

        let structured_data = data
            .to_valid_structure(vec![obj_field], DataContext::default())
            .unwrap();

        let file_contents = format_toml_frontmatter_file(structured_data);

        assert_eq!(file_contents, expected);
    }

    #[test]
    fn single_optional_object_string_field() {
        let text_field = InputField::String {
            name: "title".to_string(),
            required: InputFieldRequired(true),
        };
        let obj_field = InputField::Object {
            name: "extra".to_string(),
            input_fields: vec![text_field],
            required: InputFieldRequired(false),
        };
        let val = PostTypes::String("Hello extra World!".to_string());

        let mut fields_hm = HashMap::new();
        fields_hm.insert("title".to_string(), val);
        let obj_hm = HashMap::new();

        let data = Blob { fields: obj_hm };

        let expected = r#"+++
+++
"#;

        let structured_data = data
            .to_valid_structure(vec![obj_field], DataContext::default())
            .unwrap();

        let file_contents = format_toml_frontmatter_file(structured_data);

        assert_eq!(file_contents, expected);
    }
}
