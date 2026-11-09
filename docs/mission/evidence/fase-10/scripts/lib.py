import zipfile,json,sys
def load(p):
    return json.loads(zipfile.ZipFile(p).read('document.json'))
def walk(N,i,fn,depth=0,anc=()):
    n=N[str(i)]; fn(n,depth,anc)
    for c in n.get('children',[]) or []: walk(N,c,fn,depth+1,anc+(n,))
