from PIL import Image
import numpy as np
cases = [
 ("cap_3.2","scratch/frames/f_3.2.png",260,153,91,242),
 ("cap_50.8","scratch/frames/f_50.8.png",505,131,75,201),
 ("cap_71a","scratch/frames/f_71.4.png",97,133,76,194),
 ("cap_71b","scratch/frames/f_71.4.png",158,204,82,228),
 ("cap_98.6","scratch/frames/f_98.6.png",76,695,853,77),
 ("cap_11.1","scratch/frames/f_11.1.png",872,240,88,169),
 ("FP_24.0_xi","scratch/frames/f_24.0.png",128,108,516,260),
 ("FP_113a","scratch/frames/f_113.0.png",454,110,506,598),
 ("FP_113b","scratch/frames/f_113.0.png",338,279,386,332),
]
for name,path,x,y,w,h in cases:
    im = Image.open(path).convert("RGB")
    a = np.asarray(im.crop((x,y,x+w,y+h))).astype(np.float32)
    mx = a.max(axis=2); mn = a.min(axis=2)
    lum = 0.299*a[:,:,0]+0.587*a[:,:,1]+0.114*a[:,:,2]
    sat = np.where(mx>0,(mx-mn)/np.maximum(mx,1),0)
    bright = lum>=150
    whiteish = bright & (sat<=0.30)
    top10 = np.percentile(lum,90)
    print(f"{name:12s} px={a.shape[0]*a.shape[1]:7d} meanLum={lum.mean():6.1f} p90Lum={top10:6.1f} meanSat={sat.mean():.2f} "
          f"fracBright={bright.mean():.3f} fracWhite={whiteish.mean():.3f} fracWhiteOfBright={(whiteish.sum()/max(bright.sum(),1)):.2f}")
