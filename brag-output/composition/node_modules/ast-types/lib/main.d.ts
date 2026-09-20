import { ASTNode, Type as TypeClass, AnyType, Field } from "./types";
import { NodePath as NodePathClass } from "./node-path";
import { namedTypes } from "./gen/namedTypes";
import { builders as Builders } from "./gen/builders";
import { Visitor } from "./gen/visitor";
declare const astNodesAreEquivalent: {
    (a: any, b: any, problemPath?: any): boolean;
    assert(a: any, b: any): void;
}, builders: Builders, builtInTypes: {
    string: TypeClass<string>;
    function: TypeClass<Function>;
    array: TypeClass<any[]>;
    object: TypeClass<{
        [key: string]: any;
    }>;
    RegExp: TypeClass<RegExp>;
    Date: TypeClass<Date>;
    number: TypeClass<number>;
    boolean: TypeClass<boolean>;
    null: TypeClass<null>;
    undefined: TypeClass<undefined>;
    BigInt: TypeClass<BigInt>;
}, defineMethod: (name: any, func?: Function | undefined) => Function, eachField: (object: any, callback: (name: any, value: any) => any, context?: any) => void, finalize: () => void, getBuilderName: (typeName: any) => any, getFieldNames: (object: any) => string[], getFieldValue: (object: any, fieldName: any) => any, getSupertypeNames: (typeName: string) => string[], NodePath: import("./node-path").NodePathConstructor, Path: import("./path").PathConstructor, PathVisitor: import("./path-visitor").PathVisitorConstructor, someField: (object: any, callback: (name: any, value: any) => any, context?: any) => boolean, Type: {
    or(...types: any[]): TypeClass<any>;
    from<T>(value: any, name?: string | undefined): TypeClass<T>;
    def(typeName: string): import("./types").Def<any>;
    hasDef(typeName: string): boolean;
}, use: <T>(plugin: import("./types").Plugin<T>) => T, visit: <M = {}>(node: ASTNode, methods?: Visitor<M> | undefined) => any;
type Type<T> = TypeClass<T>;
type NodePath<N = any, V = any> = NodePathClass<N, V>;
type builders = Builders;
export { AnyType, ASTNode, astNodesAreEquivalent, builders, builtInTypes, defineMethod, eachField, Field, finalize, getBuilderName, getFieldNames, getFieldValue, getSupertypeNames, namedTypes, NodePath, Path, PathVisitor, someField, Type, use, visit, Visitor, };
