# Optimally Stupid Markup Language

Too lazy to learn Hugo or any of the other static site generators (which you should definitely use)?
Maybe your *too cool* to use those and need something so "minimal", that it's straight up stupid.
That's what **OSML** aims to be!
Don't worry.
Where **OSML** lacks functionality, it makes up for with a flexible plugin system!

## Getting Started

> This assumes you've grabbed the source, compiled it, and have the binaries added somewhere in PATH

```
osmlmk c   #  Create a brand new OSML Project
osmlmk b   #  Build your OSML Project
osmlmk p   #  Purge your compiled html  
osmlmk l   #  (WIP) Live reload changes as you go
```

Now that you have your project ready to go, take a look at the project structure.
You should see `src/`, `static/`, `dist/`
Put your `OSML` into `src/`.
Every file in `src/` will map directly to an html file in `dist/` after compilation.
For example, `src/index.osml` --> `dist/index.html`.
`static/` is for static files like images where `static/cat.gif` --> `dist/static/cat.gif`.

### Using OSML

> hello\_world.osml

```
[section
    
    [title Hello There!]

    [section
        
        Hi there my name is \\\[ 0.0 \\\] Hug bot 2000.

    ]

    *Bold*
    /Italics/
    _Underline_ 
    ~~Stikethrough~~

    + Unordered List
    ++ It's nested now.

    = Boom! It's ordered now.
    = Look at me!
]

[section

    [title That's all for now folks]
    Yeah, that's all [code OSML] offers.

]

```

### Core Block Types

TODO

