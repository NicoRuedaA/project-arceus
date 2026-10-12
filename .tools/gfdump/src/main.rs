use std::{env, fs, io::Write, path::Path};
fn u16le(b:&[u8],o:usize)->u16{u16::from_le_bytes(b[o..o+2].try_into().unwrap())}
fn u32le(b:&[u8],o:usize)->u32{u32::from_le_bytes(b[o..o+4].try_into().unwrap())}
fn u64le(b:&[u8],o:usize)->u64{u64::from_le_bytes(b[o..o+8].try_into().unwrap())}
fn is_oodle(p:&[u8])->bool{p.len()>=2 && p[0]&0x0f==0x0c && (p[0]>>4)&3==0 && p[1]&0x7f==0x06}
fn main(){
    let a:Vec<String>=env::args().collect();
    let (pack,out)=(&a[1],&a[2]);
    let b=fs::read(pack).unwrap();
    assert!(&b[0..8]==b"GFLXPACK");
    let cnt=u32le(&b,0x10) as usize; let kind=u32le(&b,0x14);
    let mut offs=vec![]; let mut p=0x18;
    while p+8<=b.len(){ let v=u64le(&b,p); if v==0||v>=b.len() as u64{break;} offs.push(v); p+=8; }
    let hash_off=p; // hash array starts right after the offsets
    let mut table=None;
    for &o in &offs{
        let end=o as usize+cnt*24;
        if end+24>b.len(){continue;}
        let first=u32le(&b,o as usize+16) as usize;
        if first==end || first==((end+15)&!15){ table=Some(o as usize); break;}
    }
    let table=table.expect("no entry table");
    fs::create_dir_all(out).unwrap();
    let mut idx=fs::File::create(Path::new(out).join("index.tsv")).unwrap();
    writeln!(idx,"i\thash\ttype\tkind\tdeclared\tsize\tflags\toffset\tmagic\tdecoded_len\tstatus").unwrap();
    eprintln!("{pack}: entries={cnt} kind={kind} offs={:x?} hash_off={hash_off:#x} table={table:#x}",offs);
    for i in 0..cnt{
        let e=table+i*24;
        let (ty,kd,decl,size,flags,off)=(u16le(&b,e),u16le(&b,e+2),u32le(&b,e+4) as usize,u32le(&b,e+8) as usize,u32le(&b,e+12),u32le(&b,e+16) as usize);
        let hash=if hash_off+8*(i+1)<=b.len(){u64le(&b,hash_off+8*i)}else{0};
        let payload=&b[off..(off+size).min(b.len())];
        let (data,status)= if is_oodle(payload){
            let mut o=vec![0u8;decl];
            match oozextract::Extractor::new().read_from_slice(payload,&mut o){
                Ok(n)=>{o.truncate(n);(o,if n==decl{"ok"}else{"short"})}
                Err(_)=>(payload.to_vec(),"oodle_err")}
        } else {(payload.to_vec(),"raw")};
        let magic:String=data.iter().take(8).map(|c| if c.is_ascii_graphic(){*c as char}else{'.'}).collect();
        writeln!(idx,"{i}\t{hash:016x}\t{ty}\t{kd}\t{decl}\t{size}\t{flags:#x}\t{off:#x}\t{magic}\t{}\t{status}",data.len()).unwrap();
        fs::write(Path::new(out).join(format!("{i:04}.bin")),&data).unwrap();
    }
}
