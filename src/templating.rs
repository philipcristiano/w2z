use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type")]
pub enum InputField {
    #[serde(alias = "string")]
    String { name: String },
    #[serde(alias = "text")]
    Text { name: String },
    #[serde(alias = "datetime")]
    DateTime { name: String, default_now: bool },
    #[serde(alias = "list")]
    List {
        name: String,
        // list_of_type: InputField,
        // Eventually support types, right now assume String type
    },
    #[serde(alias = "object")]
    Object {
        name: String,
        input_fields: Vec<InputField>,
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

impl InputEnable {
    fn markup(&self) -> &str {
        match self {
            InputEnable::Enabled => "false",
            InputEnable::Disabled => "disabled",
        }
    }
}

impl InputField {
    pub fn name(&self) -> &String {
        match self {
            InputField::Text { name } => name,
            InputField::String { name } => name,
            InputField::Object { name, .. } => name,
            InputField::List { name, .. } => name,
            InputField::DateTime { name, .. } => name,
        }
    }
    pub fn form_markup(&self, prefix: &String, form_opts: FormInputOptions) -> maud::Markup {
        match self {
            InputField::Text { name } => {
                let field_name = format!("{}[{}]", prefix, name);
                maud::html! {
                    @if form_opts.label == FormLabel::Yes { label  { (name)} }
                    textarea white-space="pre-wrap" class="border min-w-full" name={(&field_name)} {}
                }
            }
            InputField::String { name } => {
                let field_name = format!("{}[{}]", prefix, name);
                maud::html! {
                    @if form_opts.label == FormLabel::Yes { label  { (field_name)} }
                    @match form_opts.input_enable {
                        InputEnable::Disabled =>{
                            input class="border min-w-full" name={(&field_name)} disabled; {}}
                        InputEnable::Enabled =>{
                            input class="border min-w-full" name={(&field_name)} {}}

                    }
                }
            }
            InputField::List { name } => {
                let list_item_field_name = format!("{}[{}]", prefix, name);
                let item_template = InputField::String {
                    name: "".to_string(),
                };
                maud::html! {
                    span {"List items!"}
                    br {}
                    label { (&list_item_field_name)}
                    button type="button" script="on click set N to the next <div/> then set N to N.cloneNode(true) then remove .hidden from N then remove @disabled from the <input/> in N then put N after me" {"Add item"}
                    div class="hidden" {
                        (item_template.form_markup(&list_item_field_name, form_opts.without_label().disable_input()))
                        button type="button" script="on click remove me.parentElement" {"Remove"}
                    }
                }
            }
            InputField::Object { name, input_fields } => {
                let field_name = format!("{}[{}]", prefix, name);
                maud::html! {
                    @if form_opts.label == FormLabel::Yes { label  { (field_name)} }
                    @for if_field in input_fields {
                        (if_field.form_markup(&field_name, form_opts))

                    }
                }
            }
            InputField::DateTime { name, .. } => {
                let field_name = format!("{}[{}]", prefix, name);
                // TODO default shouldn't be handled by omitting things
                maud::html! {
                    label for={(&field_name)} { (&field_name)}
                    span {"Date will be set automatically"}
                }
            }
        }
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
            PostTypes::List(l) => Err(TemplateError::FieldDataError(
                "Cannot convert to list".to_string(),
            )),
            PostTypes::Object(m) => Err(TemplateError::FieldDataError(
                "Cannot convert to string".to_string(),
            )),
        }
    }
    fn value_strings(self) -> Result<Vec<String>, TemplateError> {
        match self {
            PostTypes::String(m) => Err(TemplateError::FieldDataError(
                "Cannot convert to list".to_string(),
            )),
            PostTypes::List(l) => Ok(l),
            PostTypes::Object(m) => Err(TemplateError::FieldDataError(
                "Cannot convert to list".to_string(),
            )),
        }
    }

    fn value_hm(self) -> Result<HashMap<String, PostTypes>, TemplateError> {
        match self {
            PostTypes::Object(m) => Ok(m),
            PostTypes::List(l) => Err(TemplateError::FieldDataError(
                "Cannot convert to list".to_string(),
            )),
            PostTypes::String(s) => Err(TemplateError::FieldDataError(
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
        for input_field in input_fields {
            let maybe_blob_value = hm.get(input_field.name());
            if let Some(blob_value) = maybe_blob_value {
                let field_value =
                    FieldValue::try_new(&input_field, blob_value.to_owned(), context)?;
                im.insert(input_field.name().clone(), field_value);
            } else {
                if let Some(default_fv) = FieldValue::try_new_default(&input_field, context) {
                    im.insert(input_field.name().clone(), default_fv);
                } else {
                    return Err(TemplateError::MissingField {
                        field: input_field.name().to_owned(),
                    });
                }
                // return error
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
    #[error("Field is missing: {field}")]
    EmptyField { field: String },
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
    fn try_new(
        input_field: &InputField,
        value: PostTypes,
        context: DataContext,
    ) -> Result<FieldValue, TemplateError> {
        match input_field {
            InputField::String { name } => {
                let vs = value.value_string()?;
                if vs.is_empty() {
                    return Err(TemplateError::EmptyField {
                        field: name.to_owned(),
                    });
                } else {
                    Ok(FieldValue::String(vs))
                }
            }
            InputField::Text { name } => {
                let vs = value.value_string()?;
                if vs.is_empty() {
                    return Err(TemplateError::EmptyField {
                        field: name.to_owned(),
                    });
                } else {
                    Ok(FieldValue::Text(vs))
                }
            }
            InputField::DateTime { name, default_now } => {
                let now = chrono::Utc::now();
                Ok(FieldValue::DateTime(now))
            }
            InputField::List { name } => Ok(FieldValue::List(value.value_strings()?)),
            InputField::Object { name, input_fields } => {
                let d = Blob::hm_to_valid_structure(
                    value.value_hm()?,
                    input_fields.to_owned(),
                    context,
                )?;
                Ok(FieldValue::Object(d))
            }
        }
    }
    fn try_new_default(input_field: &InputField, context: DataContext) -> Option<FieldValue> {
        match input_field {
            InputField::DateTime { name, default_now } => {
                if default_now.clone() {
                    return Some(FieldValue::DateTime(context.now));
                } else {
                    return None;
                }
            }
            _ => None,
        }
    }

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
    pub input_fields: Vec<InputField>,
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

    pub fn as_toml(&self, data: Blob) -> Result<String, TemplateError> {
        let structured_data =
            data.to_valid_structure(self.input_fields.clone(), DataContext::default())?;

        let file_contents = format_toml_frontmatter_file(structured_data);
        Ok(file_contents)
    }
}

fn format_toml_frontmatter_file(mut data: indexmap::IndexMap<String, FieldValue>) -> String {
    println!("Data {:?}", &data);
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
mod test_structure {

    use super::*;

    #[test]
    fn single_text_field() {
        let text_field = InputField::Text {
            name: "title".to_string(),
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
    fn single_text_field_empty_of_data() {
        let text_field = InputField::Text {
            name: "title".to_string(),
        };
        let mut fields_hm = HashMap::new();
        let val = PostTypes::String("".to_string());
        fields_hm.insert("title".to_string(), val);
        let data = Blob { fields: fields_hm };

        let expected = TemplateError::EmptyField {
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
        };
        let mut fields_hm = HashMap::new();
        let val = PostTypes::String("".to_string());
        fields_hm.insert("title".to_string(), val);
        let data = Blob { fields: fields_hm };

        let expected = TemplateError::EmptyField {
            field: "title".to_string(),
        };
        let maybe_structured_data = data
            .to_valid_structure(vec![field], DataContext::default())
            .unwrap_err();
        assert_eq!(maybe_structured_data, expected);
    }

    #[test]
    fn single_datetime_field() {
        let dt_field = InputField::DateTime {
            name: "date".to_string(),
            default_now: true,
        };
        let fields_hm = HashMap::new();

        let data = Blob { fields: fields_hm };
        let context = DataContext {
            now: chrono::Utc::now(),
        };
        println!("{}", toml::to_string(&context).unwrap());

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
        };
        let text_field = InputField::String {
            name: "body".to_string(),
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
        };
        let obj_field = InputField::Object {
            name: "extra".to_string(),
            input_fields: vec![text_field],
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
}
