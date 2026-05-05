// Valid/Should be highlighted as char:
// '1' 'a' 'b' '👍' '\x1b', 'notacharacter' '\'', '\\' '1''2''3' '1'notchar'2'
// Should be highlighted as lifetime specifier:
// 'a 'this_is_cool <'abc> '123
// Invalid/ should not be (fully) highlighted:
// "a", b' '   'invalid-specifier 

