// Explicit mapping: Node/ICU builds differ in their interpretation of this label.
// Undefined CP1252 bytes are deliberately not accepted as printable characters.
const extended = [0x20ac,0,0x201a,0x0192,0x201e,0x2026,0x2020,0x2021,0x02c6,0x2030,0x0160,0x2039,0x0152,0,0x017d,0,0,0x2018,0x2019,0x201c,0x201d,0x2022,0x2013,0x2014,0x02dc,0x2122,0x0161,0x203a,0x0153,0,0x017e,0x0178];
const alphabet = new Map<string,number>();
for (let byte=0;byte<256;byte++) {
  const code=byte>=128&&byte<160?extended[byte-128]!:byte;
  if (code || byte===0) alphabet.set(String.fromCodePoint(code),byte);
}
export function encodeWindows1252(text: string): Uint8Array {
  const result: number[] = [];
  for (const char of text) {
    const byte = alphabet.get(char);
    if (byte===undefined) throw new Error(`Das Zeichen „${char}“ lässt sich nicht im DATEV-Zeichensatz speichern.`);
    result.push(byte);
  }
  return Uint8Array.from(result);
}
export function checkedText(value: string, max: number, label: string): string {
  if (value.length>max || /[\u0000-\u001f\u007f-\u009f]/u.test(value)) throw new Error(`${label}: höchstens ${max} Zeichen, keine Zeilenumbrüche oder Steuerzeichen.`);
  encodeWindows1252(value);
  return value;
}
export function quote(value: string): string { return `"${value.replace(/"/g,'""')}"`; }
