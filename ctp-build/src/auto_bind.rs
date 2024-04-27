use std::collections::HashMap;
use std::path::PathBuf;

use regex::Regex;

#[derive(Debug)]
struct ClsAst {
    name: String,
    parent: Vec<ClsAst>,
    pub funcs: Vec<ClsF>,
}

impl ClsAst {
    pub fn new(cls_name: &str) -> Self {
        ClsAst {
            name: cls_name.to_string(),
            parent: vec![],
            funcs: vec![],
        }
    }

    pub fn member_to_rust(self, is_m_member_owned: bool) -> (String, String) {
        let rust_name = "Rust_".to_owned() + self.name.as_str();

        let mut funcs = vec![];
        let mut impls = vec![];
        for func in self.funcs {
            let (cdef, cimpl) = func.member_to_rust(rust_name.as_str());
            impls.push(cimpl);
            funcs.push(cdef);
        }

        let mut funcs_repl = "".to_string();
        for f in funcs {
            funcs_repl += format!("        {}\n", f.as_str()).as_str()
        }
        let des = if is_m_member_owned {
            "delete m_member;"
        } else {
            ""
        };

        impls.push(format!(
            "{}::{}({} *member) : m_member(member) {{  }};",
            rust_name, rust_name, self.name
        ));
        impls.push(format!("{}::~{}() {{ {} }};", rust_name, rust_name, des));

        let impls_repl = impls.join("\n");

        let cdef = format!(
            r#"
class {} {{
    public:
        {} *m_member;
        {}({} *member);
        ~{}();
{}
}};
"#,
            rust_name, self.name, rust_name, self.name, rust_name, funcs_repl
        );
        let cpp = format!(
            r#"
{}
"#,
            impls_repl
        );
        (cdef, cpp)
    }

    fn forward_to_rust(self) -> (String, String) {
        let rust_name = "Rust_".to_owned() + self.name.as_str();

        let mut funcs = vec![];
        let mut impls = vec![];
        let mut externcs = vec![];
        for func in self.funcs {
            if let Some((cdef, cimpl, externc)) = func.forward_to_rust(rust_name.as_str()) {
                impls.push(cimpl);
                funcs.push(cdef);
                externcs.push(externc);
            }
        }
        let mut funcs_repl = "".to_string();
        for f in funcs {
            funcs_repl += format!("        {}\n", f.as_str()).as_str()
        }

        let impls_repl = impls.join("\n");
        let externcs_repl = externcs.join("\n");

        let cdef = format!(
            r#"
class {} : {} {{
    public:
        void *m_rust;
        {}(void *rust);
        ~{}();
{}
}};
{}
extern "C" void {}_Trait_Drop(void* m_rust);
"#,
            rust_name, self.name, rust_name, rust_name, funcs_repl, externcs_repl, rust_name,
        );

        let cpp = format!(
            r#"

{}
{}::{}(void *rust) : m_rust(rust) {{}}
{}::~{}() {{ {}_Trait_Drop(m_rust); }}
"#,
            impls_repl, rust_name, rust_name, rust_name, rust_name, rust_name
        );
        (cdef, cpp)
    }
}

#[derive(Debug)]
struct ClsF {
    rtn: String,
    name: String,
    args: String,
    decl: String,
    comments: Vec<String>,
}

impl ClsF {
    pub fn new(rtn: String, name: String, args: String, decl: String, comments: Vec<&str>) -> Self {
        ClsF {
            rtn,
            name,
            args,
            decl,
            comments: comments.iter().map(|s| s.to_string()).collect(),
        }
    }

    pub fn member_to_rust(self, cls_name: &str) -> (String, String) {
        let mut rtn = self.rtn.trim().to_string();
        if rtn.starts_with("virtual") {
            rtn = rtn.strip_prefix("virtual").unwrap().trim().to_string();
        }

        let arg_pattern = Regex::new(r"^(.*)\s+([\w*]+)\s*([\]\[]*)$").unwrap();

        let mut args = vec![];
        let mut argvs = vec![];
        for arg in self.args.split(",") {
            let mut arg = arg.trim().to_string();
            if arg.len() == 0 {
                continue;
            }

            if let Some(i) = arg.find("=") {
                if i != 0 {
                    let (_, s) = arg.split_at(i);
                    arg = s.to_string()
                }
            }
            let cap = arg_pattern.captures(arg.as_str()).unwrap();

            let mut a = cap.get(1).unwrap().as_str().to_string();
            let mut v = cap.get(2).unwrap().as_str().to_string();
            let b = cap.get(3).unwrap().as_str();

            let mut trim_star = 0;

            for c in v.chars() {
                if c != '*' {
                    break;
                }
                trim_star += 1;
                a += "*"
            }
            let (_, s) = v.split_at(trim_star);
            v = s.to_string();
            args.push(format!("{} {}{}", a, v, b));
            argvs.push(v);
        }

        let args_repl = args.join(", ");
        let argvs_repl = argvs.join(", ");

        (
            format!(
                "{} {}({});",
                rtn.as_str(),
                self.name.as_str(),
                args_repl.as_str()
            ),
            format!(
                "{} {}::{}({}) {{ return m_member->{}({}); }}",
                rtn.as_str(),
                cls_name,
                self.name.as_str(),
                args_repl.as_str(),
                self.name.as_str(),
                argvs_repl.as_str()
            ),
        )
    }

    fn forward_to_rust(self, cls_name: &str) -> Option<(String, String, String)> {
        let mut rtn = self.rtn.trim().to_string();
        if rtn.starts_with("virtual") {
            rtn = rtn.strip_prefix("virtual").unwrap().trim().to_string();
        } else {
            return None;
        }

        let arg_pattern = Regex::new(r"^(.*)\s+([\w*]+)\s*([\[\]]*)$").unwrap();

        let mut args = vec![];
        let mut argvs = vec![];
        for arg in self.args.split(",") {
            let mut arg = arg.trim().to_string();
            if arg.len() == 0 {
                continue;
            }

            if let Some(i) = arg.find("=") {
                if i != 0 {
                    let (_, s) = arg.split_at(i);
                    arg = s.to_string()
                }
            }
            let cap = arg_pattern.captures(arg.as_str()).unwrap();

            let mut a = cap.get(1).unwrap().as_str().to_string();
            let mut v = cap.get(2).unwrap().as_str().to_string();
            let b = cap.get(3).unwrap().as_str();

            let mut trim_star = 0;

            for c in v.chars() {
                if c != '*' {
                    break;
                }
                trim_star += 1;
                a += "*"
            }
            let (_, s) = v.split_at(trim_star);
            v = s.to_string();
            args.push(format!("{} {}{}", a, v, b));
            argvs.push(v);
        }

        let args_repl = args.join(", ");

        argvs.insert(0, "m_rust".to_string());
        let argvs_repl = argvs.join(", ");

        args.insert(0, "void *m_rust".to_string());
        let args_repl_extern = args.join(", ");

        let trait_name = format!("{}_Trait", cls_name);
        let forward_func = format!("{}_{}", trait_name, self.name);

        Some((
            format!("{} {}({}) override;", rtn, self.name, args_repl),
            format!(
                "{} {}::{}({}) {{ return {}({}); }}",
                rtn, cls_name, self.name, args_repl, forward_func, argvs_repl
            ),
            format!(
                "extern \"C\" {} {}({});",
                rtn, forward_func, args_repl_extern
            ),
        ))
    }
}

fn walk_hpp(content: &str) -> HashMap<&str, ClsAst> {
    let mut meta = HashMap::new();

    let class_start = Regex::new(r"class.*\s+(\w+)\s*\{").unwrap();
    let func_comment = Regex::new(r"^\s*//(.*)\s*$").unwrap();
    let member_func = Regex::new(r"^\s*(.*)\s+([\w*]+)\((.*)\)(.*);\s*$").unwrap();

    for cap in class_start.captures_iter(content) {
        let cls_name = cap.get(1).unwrap().as_str();
        let mut cls_ast = ClsAst::new(cls_name);

        let contents = content[cap.get(0).unwrap().end()..].split("\n");
        let mut comments = vec![];

        for line in contents {
            if let Some(cap) = func_comment.captures(line) {
                // 收集注释
                comments.push(cap.get(1).unwrap().as_str().strip_prefix("/").unwrap());
            } else if let Some(cap) = member_func.captures(line) {
                let rtn = cap.get(1).unwrap().as_str();
                let funcname = cap.get(2).unwrap().as_str();
                let args = cap.get(3).unwrap().as_str();
                let decl = cap.get(4).unwrap().as_str();

                let left = decl.matches("{").count();
                let right = decl.matches('}').count();
                assert_eq!(left, right, "{}", decl);

                let mut curr = 0;
                let mut rtn = rtn.to_string();
                for c in funcname.chars() {
                    if c != '*' {
                        break;
                    }
                    curr += 1;
                    rtn += "*";
                }

                let funcname = funcname[curr..].to_string();
                if rtn.trim().starts_with("static") {
                    /*pass*/
                } else {
                    cls_ast.funcs.push(ClsF::new(
                        rtn,
                        funcname,
                        args.to_string(),
                        decl.to_string(),
                        comments,
                    ));
                }
                comments = vec![];
            } else {
                let left = line.matches("{").count();
                let right = line.matches('}').count();
                if left == 0 && right == 1 {
                    break;
                }
            }
        }

        meta.insert(cls_name, cls_ast);
    }

    meta
}

pub fn port_ctp_md(include_root: &PathBuf) -> (String, String) {
    let buf = std::fs::read_to_string(include_root.join("ThostFtdcMdApi.h")).unwrap();
    let mut meta = walk_hpp(buf.as_str());

    let c1 = meta.remove("CThostFtdcMdApi").unwrap();
    let (hpp_buf, cpp_buf) = c1.member_to_rust(false);

    let c2 = meta.remove("CThostFtdcMdSpi").unwrap();
    let (hpp, cpp) = c2.forward_to_rust();

    (hpp_buf + hpp.as_str(), cpp_buf + cpp.as_str())
}

pub fn port_ctp_td(include_root: &PathBuf) -> (String, String) {
    let buf = std::fs::read_to_string(include_root.join("ThostFtdcTraderApi.h")).unwrap();
    let mut meta = walk_hpp(buf.as_str());

    let c1 = meta.remove("CThostFtdcTraderApi").unwrap();
    let (hpp_buf, cpp_buf) = c1.member_to_rust(false);

    let c2 = meta.remove("CThostFtdcTraderSpi").unwrap();
    let (hpp, cpp) = c2.forward_to_rust();

    (hpp_buf + hpp.as_str(), cpp_buf + cpp.as_str())
}

pub fn auto_bind(include_root: &PathBuf, hpp_file: &PathBuf, cpp_file: &PathBuf) {
    let mut hpp_buf: String = r#"#pragma once

#include "ThostFtdcUserApiDataType.h"
#include "ThostFtdcUserApiStruct.h"
#include "ThostFtdcTraderApi.h"
#include "ThostFtdcMdApi.h"
    "#
    .to_string();

    let mut cpp_buf: String = r#"
#include "wrapper.hpp"

    "#
    .to_string();

    let (hpp, cpp) = port_ctp_md(include_root);
    hpp_buf += hpp.as_str();
    hpp_buf += "\n";

    cpp_buf += cpp.as_str();
    cpp_buf += "\n";

    let (hpp, cpp) = port_ctp_td(include_root);
    hpp_buf += hpp.as_str();
    hpp_buf += "\n";

    cpp_buf += cpp.as_str();
    cpp_buf += "\n";

    std::fs::write(&hpp_file, &hpp_buf).expect("Fail to write hpp!");
    std::fs::write(&cpp_file, &cpp_buf).expect("Fail to write hpp!");
}
