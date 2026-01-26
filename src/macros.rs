#[macro_export]
macro_rules! kpairs {
	($(($key:expr => $func:expr)),+ $(,)?) => {
			vec![ $(TKeePair {
            key: stringify!($key).to_string(),
            func: stringify!($func).to_string(),
        }),+
      ]
    };
}
