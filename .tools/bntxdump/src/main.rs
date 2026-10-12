use std::{env, fs, io::Write};
use tegra_swizzle::surface::{deswizzle_surface, BlockDim};
fn u16le(b:&[u8],o:usize)->u16{u16::from_le_bytes(b[o..o+2].try_into().unwrap())}
fn u32le(b:&[u8],o:usize)->u32{u32::from_le_bytes(b[o..o+4].try_into().unwrap())}
fn u64le(b:&[u8],o:usize)->u64{u64::from_le_bytes(b[o..o+8].try_into().unwrap())}
fn main(){
    // usage: bntxdump <entry.bin> <out_prefix>   -> writes <prefix>_<texname>.f32 (little-endian floats) for float formats, prints stats
    let a:Vec<String>=env::args().collect();
    let b=fs::read(&a[1]).unwrap();
    assert!(&b[0..4]==b"BNTX");
    let cnt=u32le(&b,0x24) as usize; let tab=u64le(&b,0x28) as usize;
    for i in 0..cnt{
        let brti=u64le(&b,tab+8*i) as usize;
        let tile=u16le(&b,brti+0x12); let mips=u16le(&b,brti+0x16) as u32; let fmt=u32le(&b,brti+0x1c);
        let w=u32le(&b,brti+0x24); let h=u32le(&b,brti+0x28); let depth=u32le(&b,brti+0x2c).max(1); let arr=u32le(&b,brti+0x30).max(1);
        let name_off=u64le(&b,brti+0x60) as usize; let nl=u16le(&b,name_off) as usize;
        let name=String::from_utf8_lossy(&b[name_off+2..name_off+2+nl]).to_string();
        let ptab=u64le(&b,brti+0x70) as usize; let mip0=u64le(&b,ptab) as usize;
        let bpp=match fmt>>8 {0x0a=>2,0x14=>4,0x02=>1,0x0b=>4,_=>0};
        eprintln!("{name}: {w}x{h} fmt={fmt:#x} tile={tile} mips={mips} bpp={bpp} depth={depth} arr={arr}");
        if bpp==0 {continue;}
        let src=&b[mip0..];
        let lin= if tile==1 { src.to_vec() } else {
            deswizzle_surface(w,h,depth,src,BlockDim{width:std::num::NonZeroU32::new(1).unwrap(),height:std::num::NonZeroU32::new(1).unwrap(),depth:std::num::NonZeroU32::new(1).unwrap()},None,bpp,mips,arr).expect("deswizzle")
        };
        let n=(w*h) as usize;
        let mut vals=Vec::with_capacity(n);
        for p in 0..n{
            let v:f32= match (fmt>>8, fmt&0xff) {
                (0x0a,0x05)=>half::f16::from_bits(u16le(&lin,p*2)).to_f32(),
                (0x0a,0x01)=>u16le(&lin,p*2) as f32/65535.0,
                (0x14,0x05)=>f32::from_le_bytes(lin[p*4..p*4+4].try_into().unwrap()),
                (0x02,_)=>lin[p] as f32/255.0,
                _=>f32::NAN};
            vals.push(v);
        }
        let (mn,mx)=vals.iter().fold((f32::MAX,f32::MIN),|(a,b),&v| if v.is_finite(){(a.min(v),b.max(v))}else{(a,b)});
        let mean=vals.iter().filter(|v|v.is_finite()).sum::<f32>()/n as f32;
        println!("{name}\t{w}\t{h}\t{fmt:#x}\tmin={mn}\tmax={mx}\tmean={mean}");
        let mut f=fs::File::create(format!("{}_{}.f32",a[2],name)).unwrap();
        for v in &vals{f.write_all(&v.to_le_bytes()).unwrap();}
    }
}
