#!/usr/bin/env python3
"""Rasterize Termcraft ANSI audit captures; requires Pillow, never opens a save.

Supports the cursor and SGR subset emitted by the renderer. This is an offline
software capture, not a full terminal emulator or a screen-latency measurement.
"""
import argparse
import codecs
import re
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

palette=[(0,0,0),(170,0,0),(0,170,0),(170,85,0),(0,0,170),(170,0,170),(0,170,170),(170,170,170),(85,85,85),(255,85,85),(85,255,85),(255,255,85),(85,85,255),(255,85,255),(85,255,255),(255,255,255)]
def indexed(n):
 if n<16:return palette[n]
 if n<232:
  n-=16;v=[0,95,135,175,215,255];return(v[n//36],v[n//6%6],v[n%6])
 return (8+(n-232)*10,)*3
class Screen:
 def __init__(self,w,h):
  self.w,self.h=w,h;self.cells=[[(' ',palette[7],palette[0]) for _ in range(w)] for _ in range(h)];self.x=self.y=0;self.fg=palette[7];self.bg=palette[0];self.pending='';self.decoder=codecs.getincrementaldecoder('utf8')('replace')
 def feed(self,data):
  s=self.pending+self.decoder.decode(data);self.pending='';i=0
  while i<len(s):
   c=s[i]
   if c=='\x1b':
    if i+1==len(s):self.pending=s[i:];break
    if s[i+1]=='[':
     m=re.match(r'\x1b\[([0-9;?<>=!]*)([@-~])',s[i:])
     if not m:self.pending=s[i:];break
     raw,cmd=m.group(1),m.group(2);i+=len(m.group());args=[int(v or 0) for v in raw.split(';')] if not raw.startswith(('?','<','>','=')) else []
     if cmd in ('H','f'):self.y=max(0,(args[0] if args else 1)-1);self.x=max(0,(args[1] if len(args)>1 else 1)-1)
     elif cmd=='J' and args==[2]:self.cells=[[(' ',self.fg,self.bg) for _ in range(self.w)] for _ in range(self.h)]
     elif cmd=='m':
      j=0
      while j<len(args):
       a=args[j]
       if a==0:self.fg,self.bg=palette[7],palette[0]
       elif 30<=a<=37:self.fg=palette[a-30]
       elif 40<=a<=47:self.bg=palette[a-40]
       elif 90<=a<=97:self.fg=palette[a-90+8]
       elif 100<=a<=107:self.bg=palette[a-100+8]
       elif a==39:self.fg=palette[7]
       elif a==49:self.bg=palette[0]
       elif a in (38,48) and j+1<len(args):
        color=None
        if args[j+1]==2 and j+4<len(args):color=tuple(args[j+2:j+5]);j+=4
        elif args[j+1]==5 and j+2<len(args):color=indexed(args[j+2]);j+=2
        if color is not None:
         if a==38:self.fg=color
         else:self.bg=color
       j+=1
     continue
    i+=2;continue
   if c=='\r':self.x=0
   elif c=='\n':self.y=min(self.h-1,self.y+1)
   elif ord(c)>=32:
    if 0<=self.x<self.w and 0<=self.y<self.h:self.cells[self.y][self.x]=(c,self.fg,self.bg)
    self.x+=1
   i+=1
 def text(self):return '\n'.join(''.join(c[0] for c in row) for row in self.cells)
 def png(self,path):
  image=Image.new('RGB',(self.w*9,self.h*17));draw=ImageDraw.Draw(image);font=ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf',14)
  for y,row in enumerate(self.cells):
   for x,(char,fg,bg) in enumerate(row):
    draw.rectangle((x*9,y*17,x*9+8,y*17+16),fill=bg)
    draw.text((x*9,y*17-1),char,font=font,fill=fg)
  image.save(path)

def raster(screen):
    image = Image.new("RGB", (screen.w * 9, screen.h * 18))
    draw = ImageDraw.Draw(image)
    font = ImageFont.truetype("DejaVuSansMono.ttf", 14)
    for y, row in enumerate(screen.cells):
        for x, (char, fg, bg) in enumerate(row):
            left, top = x * 9, y * 18
            draw.rectangle((left, top, left + 8, top + 17), fill=bg)
            if char == "▀":
                draw.rectangle((left, top, left + 8, top + 8), fill=fg)
            elif char == "▄":
                draw.rectangle((left, top + 9, left + 8, top + 17), fill=fg)
            elif char == "█":
                draw.rectangle((left, top, left + 8, top + 17), fill=fg)
            else:
                draw.text((left, top), char, font=font, fill=fg)
    return image


def main():
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument("input", type=Path)
    args.add_argument("output", type=Path)
    args = args.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    for path in sorted(args.input.glob("*.ansi")):
        # Motion captures are incremental frames, not standalone screen dumps.
        if path.name.startswith("motion-"):
            continue
        size = re.search(r"-(\d+)x(\d+)$", path.stem)
        width, height = map(int, size.groups()) if size else (160, 48)
        screen = Screen(width, height)
        screen.feed(path.read_bytes())
        raster(screen).save(args.output / (path.stem + ".png"))
    scenes = ["humanoids", "farm", "small", "nether", "props", "cave", "night"]
    font = ImageFont.truetype("DejaVuSansMono.ttf", 20)
    sheet = Image.new("RGB", (1440, 4 * 297), (15, 20, 29))
    draw = ImageDraw.Draw(sheet)
    for index, name in enumerate(scenes):
        file = args.output / (name + "-160x48.png")
        if not file.exists():
            continue
        x, y = index % 2 * 720, index // 2 * 297
        draw.text((x + 12, y + 5), name, font=font, fill=(203, 221, 214))
        frame = Image.open(file).crop((0, 18 * 10, 1440, 18 * 40))
        sheet.paste(frame.resize((720, 270), Image.Resampling.NEAREST), (x, y + 27))
    sheet.save(args.output / "entities.png")


if __name__ == "__main__":
    main()
