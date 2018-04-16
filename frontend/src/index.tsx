// Global CSS
import '../styles/theme.scss';

import * as React from 'react';
import * as ReactDOM from 'react-dom';
import axios from 'axios';

class Answers extends React.Component {
  props: {
    answers: any,
    logged_in: boolean,
  };

  state = {
    logged_in: this.props.logged_in,
    answers: this.props.answers,
  };

  render(): React.ReactNode {
    const self = this;
    return (
      <div id="content-render">
        <div id="header">
          <h1>answered this</h1>
          {self.state.logged_in ?
            <div className="caption">
              <a href="/new">Submit new answer?</a>
            </div>
            : null}
        </div>
        {this.state.answers.map((item, i) => {
          return (
            <div
              className={`answer ${item.answered ? 'answered' : 'collapsed'}`}
              key={item.id}>
              <div
                className="title"
                dangerouslySetInnerHTML={{__html: item.title}}
                style={{
                  cursor: 'pointer',
                }}
                onClick={(e) => {
                  let updatedAnswers = JSON.parse(JSON.stringify(self.state.answers));
                  updatedAnswers[i].answered = !updatedAnswers[i].answered;
                  self.setState({answers: updatedAnswers});
                  e.preventDefault();
                }}
              />
              <div className="as-of">
                <a href={`/edit?id=${item.id}`}>As of {item.asof}.</a>
              </div>

              <div
                className="content"
                dangerouslySetInnerHTML={{__html: item.content}}
              />
            </div>
          );
        })}
        <div id="footer">
          &copy; 2018 AnsweredThis.com
        </div>
      </div>
    );
  }
}

function start() {
  // Create the editor frame.
  axios.get('/api/answers/')
  .then((res) => {
    ReactDOM.render(
      <Answers
        answers={res.data.answers}
        logged_in={res.data.logged_in}
      />,
      document.querySelector('#content')!,
    );
  })
  .catch((err) => {
    console.log(err);
  });
}

document.addEventListener('DOMContentLoaded', start);
