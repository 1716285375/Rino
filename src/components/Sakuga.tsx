import {createElement,useEffect,useRef} from 'react';
import type {ReactNode} from 'react';

type InteractiveProps={children:ReactNode;onClick?:()=>void;variant?:'secondary'|'primary';disabled?:boolean;className?:string};
/** Original Sakuga Web Component, events attached natively to avoid React custom-element differences. */
export function SakugaButton({children,onClick,variant,disabled,className}:InteractiveProps){
 const ref=useRef<HTMLElement|null>(null);
 useEffect(()=>{const el=ref.current;if(!el||!onClick)return;const callback=(e:Event)=>{e.preventDefault();onClick()};el.addEventListener('click',callback);return()=>el.removeEventListener('click',callback)},[onClick]);
 useEffect(()=>{if(ref.current) (ref.current as unknown as {disabled:boolean}).disabled=!!disabled},[disabled]);
 return createElement('sakuga-button',{ref,className,variant:variant==='secondary'?'secondary':undefined,role:'button',tabIndex:0},children);
}
export function SakugaToggle({value,onChange,label}:{value:boolean;onChange:(v:boolean)=>void;label:string}){
 const ref=useRef<HTMLElement|null>(null);
 useEffect(()=>{const e=ref.current as unknown as {checked:boolean}|null;if(e)e.checked=value},[value]);
 useEffect(()=>{const e=ref.current;if(!e)return;const callback=()=>onChange((e as unknown as {checked:boolean}).checked);e.addEventListener('change',callback);return()=>e.removeEventListener('change',callback)},[onChange]);
 return createElement('sakuga-toggle',{ref,label});
}
export function SakugaTabs({values,selected,onChange}:{values:string[];selected:number;onChange:(i:number)=>void}){
 const ref=useRef<HTMLElement|null>(null);
 useEffect(()=>{const e=ref.current as unknown as {selected:number}|null;if(e)e.selected=selected},[selected]);
 useEffect(()=>{const e=ref.current;if(!e)return;const callback=(event:Event)=>{const v=(event as CustomEvent<{index:number}>).detail?.index;if(Number.isInteger(v))onChange(v)};e.addEventListener('change',callback);return()=>e.removeEventListener('change',callback)},[onChange]);
 return createElement('sakuga-tabs',{ref,label:'内容切换'},...values.map((v,i)=>createElement('option',{key:i,value:String(i)},v)));
}
