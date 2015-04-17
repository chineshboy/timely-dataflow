#![feature(core)]

/* Based on src/main.rs from timely-dataflow by Frank McSherry,
*
* The MIT License (MIT)
*
* Copyright (c) 2014 Frank McSherry
*
* Permission is hereby granted, free of charge, to any person obtaining a copy
* of this software and associated documentation files (the "Software"), to deal
* in the Software without restriction, including without limitation the rights
* to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
* copies of the Software, and to permit persons to whom the Software is
* furnished to do so, subject to the following conditions:
*
* The above copyright notice and this permission notice shall be included in all
* copies or substantial portions of the Software.
*
* THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
* IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
* FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
* AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
* LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
* OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
* SOFTWARE.
*/

extern crate timely;
extern crate core;
extern crate columnar;

use core::fmt::Debug;

use std::cell::RefCell;
use std::hash::Hash;

use timely::communication::{Communicator, ThreadCommunicator};
use timely::communication::channels::Data;
use timely::progress::nested::product::Product;
use timely::progress::nested::builder::Builder as SubgraphBuilder;
use timely::progress::nested::Summary::Local;
use timely::progress::scope::Scope;
use timely::progress::graph::{Graph, Root};
use timely::example::*;
use timely::example::distinct::DistinctExtensionTrait;

use columnar::Columnar;

fn main() {
    _distinct(ThreadCommunicator);
}

fn _distinct<C: Communicator>(communicator: C) {

    let mut root = Root::new(communicator);

    let (mut input1, mut input2) = {
        let borrow = root.builder();
        let mut graph = SubgraphBuilder::new(&borrow);
        let (input1, input2) = {
            let builder = graph.builder();

            // try building some input scopes
            let (input1, mut stream1) = builder.new_input::<u64>();
            let (input2, mut stream2) = builder.new_input::<u64>();

            // prepare some feedback edges
            let (mut feedback1, mut feedback1_output) = builder.feedback(Product::new((), 1000000), Local(1));
            let (mut feedback2, mut feedback2_output) = builder.feedback(Product::new((), 1000000), Local(1));

            // build up a subgraph using the concatenated inputs/feedbacks
            let (mut egress1, mut egress2) = _create_subgraph(&mut stream1.concat(&mut feedback1_output),
                                                              &mut stream2.concat(&mut feedback2_output));

            // connect feedback sources. notice that we have swapped indices ...
            feedback1.connect_input(&mut egress2);
            feedback2.connect_input(&mut egress1);

            (input1, input2)
        };
        graph.seal();

        (input1, input2)
    };

    root.step();

    // move some data into the dataflow graph.
    input1.send_messages(&Product::new((), 0), vec![1u64]);
    input2.send_messages(&Product::new((), 0), vec![2u64]);

    // see what everyone thinks about that ...
    root.step();

    input1.advance(&Product::new((), 0), &Product::new((), 1000000));
    input2.advance(&Product::new((), 0), &Product::new((), 1000000));
    input1.close_at(&Product::new((), 1000000));
    input2.close_at(&Product::new((), 1000000));

    // spin
    while root.step() { }
}

fn _create_subgraph<'a, G, D>(source1: &mut Stream<'a, G, D>, source2: &mut Stream<'a, G, D>) ->
    (Stream<'a, G, D>, Stream<'a, G, D>)
where G: Graph+'a, D: Data+Hash+Eq+Debug+Columnar {

    let mut subgraph = SubgraphBuilder::<_, u64>::new(source1.graph);
    let result = {
        let subgraph_builder = subgraph.builder();

        (
            source1.enter(&subgraph_builder).distinct().leave(),
            source2.enter(&subgraph_builder).leave()
        )
    };
    subgraph.seal();

    result
}
