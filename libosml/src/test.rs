use super::*;

#[test]
fn test_parsers() {
    let my_osml = r"

[abc Hello World]

[nested
    [nested Ok?]
]

[plugin

Hello to everyone who is reading this.
This sentence should be on the same line.

Although, this one will not be.
Hopefully, *all* \~~tests\~~ /will/ be _green_, and all will be good.

]

[lists

    + FirstElement + 10
+   Second Element
++Nested Element
This is just normal text.

]

";

    let expected_result = "\
<html>\
    <head>head</head>\
    <body>\
        body\
        <div class='abc'>Hello World</div>\
        <div class='nested'><div class='nested'>Ok?</div></div>\
        <plugin>\
            <br><br>\
            Hello to everyone who is reading this. \
            This sentence should be on the same line. <br><br>\
            Although, this one will not be. \
            Hopefully, <b>all</b> ~~tests~~ <i>will</i> be <u>green</u>, and all will be good. <br><br>\
        </plugin>\
        <div class='lists'>\
            <br><br>\
            <ul>\
                <li>FirstElement + 10 </li>\
                <li>Second Element </li>\
                <ul>\
                    <li>Nested Element </li>\
                </ul>\
            </ul>\
            This is just normal text. <br><br>\
        </div>\
    </body>\
</html>";

    fn my_plugin(
        lines: &Vec<Vec<char>>,
        mut line: Line,
        mut pos: Pos,
        mut output: String,
        ctx: &Context,
    ) -> Result<(Line, Pos, String)> {
        output = format!("{}<plugin>", output);
        let mut last_list_was_ordered = None;
        loop {
            let (done, nline, npos, noutput, nlast_list_was_ordered) =
                parse_text_line(lines, line, pos, output, ctx, true, last_list_was_ordered)?;

            line = nline;
            pos = npos;
            output = noutput;
            last_list_was_ordered = nlast_list_was_ordered;
            if done {
                break;
            }
        }
        output = format!("{}</plugin>", output);
        Ok((line, pos, output))
    }

    let res = parse(
        my_osml.to_string(),
        Context {
            plugins: HashMap::from([("plugin".to_string(), my_plugin as ExtCallback)]),
            head_insert: "head".to_string(),
            body_insert: "body".to_string(),
        },
    )
    .unwrap();
    assert_eq!(res, expected_result);
}
