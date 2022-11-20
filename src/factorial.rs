use pyo3::once_cell::GILOnceCell;
use pyo3::{
    types::{PyList, PyModule, PyTuple},
    Py, PyAny, PyErr, PyResult, Python,
};
use rug::Integer;

static ASYNC_FUNC: GILOnceCell<Py<PyAny>> = GILOnceCell::new();
static FACTORIAL_FUNC: GILOnceCell<Py<PyAny>> = GILOnceCell::new();
static PY_UTILS: &str = include_str!("../py_utils/utils.py");

fn set_python_path(py: Python<'_>) {
    let py_utils_path = env!("CARGO_MANIFEST_DIR");
    let venv_path = "/Users/vamshiaruru/Library/Caches/pypoetry/virtualenvs/axum-practice-x0KsYOAf-py3.9/lib/python3.9/site-packages";
    let syspath: &PyList = py
        .import("sys")
        .unwrap()
        .getattr("path")
        .unwrap()
        .downcast::<PyList>()
        .unwrap();
    if !syspath.contains(py_utils_path).unwrap() {
        syspath.insert(0, py_utils_path).unwrap();
    }
    if !syspath.contains(venv_path).unwrap() {
        syspath.insert(0, venv_path).unwrap();
    }
    syspath.insert(0, "/Users/vamshiaruru/Library/Caches/pypoetry/virtualenvs/axum-practice-x0KsYOAf-py3.9/lib/python3.9/site-packages").unwrap();
}

fn load_factorial_func(py: Python<'_>) -> Py<PyAny> {
    println!("Loading factorial func!");
    set_python_path(py);
    PyModule::from_code(py, PY_UTILS, "", "")
        .unwrap()
        .getattr("factorial")
        .unwrap()
        .into()
}

fn load_async_func(py: Python<'_>) -> Py<PyAny> {
    set_python_path(py);
    PyModule::from_code(py, PY_UTILS, "", "")
        .unwrap()
        .getattr("async_op")
        .unwrap()
        .into()
}

pub fn calculate_factorial(number: usize) -> Integer {
    let mut res = Integer::from(1);
    for index in 1..number {
        res *= index;
    }
    res
}

pub fn calculate_factorial_py(number: usize) -> Result<Py<PyAny>, PyErr> {
    Python::with_gil(|py| -> PyResult<Py<PyAny>> {
        let func = FACTORIAL_FUNC.get_or_init(py, || load_factorial_func(py));
        func.call1(py, PyTuple::new(py, [number]))
    })
}

pub async fn async_operation() {
    let fut = Python::with_gil(|py| {
        let func = ASYNC_FUNC.get_or_init(py, || load_async_func(py));
        pyo3_asyncio::tokio::into_future(func.call0(py).unwrap().as_ref(py))
    })
    .unwrap();
    fut.await.unwrap();
}
